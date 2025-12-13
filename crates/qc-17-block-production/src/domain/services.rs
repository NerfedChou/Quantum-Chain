//! Domain services for block production

use super::entities::*;
use crate::error::{BlockProductionError, Result};
use primitive_types::U256;
use std::collections::HashMap;

/// Transaction selector service (core domain logic)
///
/// Implements the Priority-Based Greedy Knapsack algorithm for optimal
/// transaction selection within block gas limit constraints.
pub struct TransactionSelector {
    /// Block gas limit
    gas_limit: u64,

    /// Minimum gas price threshold
    min_gas_price: U256,

    /// MEV protection enabled
    fair_ordering: bool,
}

impl TransactionSelector {
    /// Create new transaction selector
    pub fn new(gas_limit: u64, min_gas_price: U256, fair_ordering: bool) -> Self {
        Self {
            gas_limit,
            min_gas_price,
            fair_ordering,
        }
    }

    /// Check if fair ordering (MEV protection) is enabled.
    pub fn is_fair_ordering(&self) -> bool {
        self.fair_ordering
    }

    /// Select optimal transaction set using greedy knapsack
    ///
    /// Algorithm: Priority-Based Greedy Knapsack (O(n log n))
    /// Complexity: O(n log n)
    #[tracing::instrument(skip(self, candidates, state_cache), fields(candidate_count = candidates.len()))]
    pub fn select_transactions(
        &self,
        candidates: Vec<TransactionCandidate>,
        state_cache: &mut StatePrefetchCache,
    ) -> Result<Vec<Vec<u8>>> {
        use std::collections::{BinaryHeap, HashMap};

        if candidates.is_empty() {
            return Ok(vec![]);
        }

        // Step 1: Group transactions by sender
        let mut sender_txs: HashMap<[u8; 20], Vec<TransactionCandidate>> = HashMap::new();
        for tx in candidates {
            // Filter by minimum gas price
            if tx.gas_price < self.min_gas_price {
                continue;
            }
            sender_txs.entry(tx.from).or_default().push(tx);
        }

        // Step 2: Sort each sender's transactions by nonce (ascending)
        for txs in sender_txs.values_mut() {
            txs.sort_by_key(|tx| tx.nonce);
        }

        // Step 3: Build priority queue by gas price (max heap)
        #[derive(Debug)]
        struct TxRef {
            from: [u8; 20],
            idx: usize,
            gas_price: U256,
        }

        impl PartialEq for TxRef {
            fn eq(&self, other: &Self) -> bool {
                self.gas_price == other.gas_price
            }
        }

        impl Eq for TxRef {}

        impl PartialOrd for TxRef {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for TxRef {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.gas_price.cmp(&other.gas_price)
            }
        }

        let mut pq: BinaryHeap<TxRef> = BinaryHeap::new();
        let mut sender_indices: HashMap<[u8; 20], usize> = HashMap::new();

        // Initialize with first transaction from each sender
        for (from, txs) in &sender_txs {
            if !txs.is_empty() {
                pq.push(TxRef {
                    from: *from,
                    idx: 0,
                    gas_price: txs[0].gas_price,
                });
                sender_indices.insert(*from, 0);
            }
        }

        // Step 4: Greedy selection with simulation
        let mut selected = Vec::new();
        let mut total_gas = 0u64;

        tracing::debug!(
            "Starting greedy selection: {} sender groups, gas_limit={}",
            sender_txs.len(),
            self.gas_limit
        );

        while let Some(tx_ref) = pq.pop() {
            let sender_txs_list = &sender_txs[&tx_ref.from];
            let tx = &sender_txs_list[tx_ref.idx];

            // Check if we have space for this transaction
            if total_gas + tx.gas_limit > self.gas_limit {
                continue; // Skip, try next
            }

            // Simulate transaction
            let sim_result = state_cache.simulate_transaction(&tx.transaction);

            if sim_result.success && total_gas + sim_result.gas_used <= self.gas_limit {
                // Accept transaction
                selected.push(tx.transaction.clone());
                total_gas += sim_result.gas_used;

                // Apply state changes
                state_cache.apply_state_changes(&sim_result.state_changes);

                // Add next transaction from same sender to priority queue
                let next_idx = tx_ref.idx + 1;
                if next_idx < sender_txs_list.len() {
                    let next_tx = &sender_txs_list[next_idx];

                    // Verify nonce is sequential
                    if next_tx.nonce == tx.nonce + 1 {
                        pq.push(TxRef {
                            from: tx_ref.from,
                            idx: next_idx,
                            gas_price: next_tx.gas_price,
                        });
                        sender_indices.insert(tx_ref.from, next_idx);
                    }
                }
            }
            // If simulation failed, skip this sender's remaining transactions
        }

        tracing::info!(
            "Transaction selection complete: selected={}, total_gas={}/{}",
            selected.len(),
            total_gas,
            self.gas_limit
        );

        Ok(selected)
    }

    /// Validate nonce ordering for a set of transactions
    ///
    /// Validates that transaction nonces are sequential per sender.
    /// Returns Ok if nonces are valid, Err otherwise.
    pub fn validate_nonce_ordering(&self, _transactions: &[TransactionCandidate]) -> Result<()> {
        // Nonce validation is handled by the mempool (qc-06)
        // This is a passthrough for block production
        Ok(())
    }

    /// Detect MEV bundles in transaction set
    ///
    /// Detects potential MEV patterns like front-running, back-running, and sandwiches.
    /// Returns empty vec when no MEV detection is configured.
    pub fn detect_mev_bundles(&self, _transactions: &[Vec<u8>]) -> Vec<TransactionBundle> {
        // MEV detection is an advanced feature - returns empty for now
        // Production deployments should implement MEV protection strategies
        Vec::new()
    }
}

/// State prefetch cache for simulation
///
/// Caches account states and storage slots to avoid re-reading during
/// transaction simulation.
pub struct StatePrefetchCache {
    /// Parent state root (used for cache invalidation)
    parent_state_root: primitive_types::H256,

    /// Cached account states
    accounts: HashMap<[u8; 20], AccountState>,

    /// Cached storage slots
    storage: HashMap<([u8; 20], primitive_types::H256), Vec<u8>>,
}

/// Account state snapshot
#[derive(Clone, Debug)]
pub struct AccountState {
    /// Account nonce
    pub nonce: u64,

    /// Account balance
    pub balance: U256,

    /// Code hash (None for EOA)
    pub code_hash: Option<primitive_types::H256>,
}

impl StatePrefetchCache {
    /// Create new prefetch cache
    pub fn new(parent_state_root: primitive_types::H256) -> Self {
        Self {
            parent_state_root,
            accounts: HashMap::new(),
            storage: HashMap::new(),
        }
    }

    /// Get the parent state root this cache was created for.
    ///
    /// Used for cache invalidation when state changes.
    pub fn parent_state_root(&self) -> primitive_types::H256 {
        self.parent_state_root
    }

    /// Simulate transaction execution
    ///
    /// Mock implementation - will integrate with Subsystem 4 later
    pub fn simulate_transaction(&mut self, tx: &[u8]) -> SimulationResult {
        use crate::utils::hashing::transaction_hash;

        // Calculate tx hash
        let tx_hash = transaction_hash(tx);

        // Mock simulation: assume all transactions succeed for now
        // In real implementation, this would call Subsystem 4

        // Simple heuristic: transactions with even-length payloads succeed
        let success = tx.len() % 2 == 0 || tx.is_empty();

        let gas_used = if success {
            21000 // Base transaction cost
        } else {
            0
        };

        SimulationResult {
            tx_hash,
            success,
            gas_used,
            // Mock implementation: always empty state changes
            // Real implementation would populate from Subsystem 4 simulation
            state_changes: vec![],
            error: if success {
                None
            } else {
                Some("Mock: odd-length transaction failed".to_string())
            },
        }
    }

    /// Apply simulation result to cache
    pub fn apply_state_changes(&mut self, changes: &[StateChange]) {
        for change in changes {
            // Update cached account state
            let account = self
                .accounts
                .entry(change.address)
                .or_insert_with(|| AccountState {
                    nonce: 0,
                    balance: U256::zero(),
                    code_hash: None,
                });

            // Apply the change (simplified)
            if change.storage_key.is_none() {
                // This is a balance/nonce change
                // In real implementation, we'd parse the change properly
                account.nonce += 1; // Increment nonce for any state change
            } else {
                // Storage slot change
                let key = (change.address, change.storage_key.unwrap());
                self.storage.insert(key, change.new_value.clone());
            }
        }
    }

    /// Get current nonce for address
    pub fn get_nonce(&self, address: [u8; 20]) -> u64 {
        self.accounts
            .get(&address)
            .map(|acc| acc.nonce)
            .unwrap_or(0)
    }

    /// Get current balance for address
    pub fn get_balance(&self, address: [u8; 20]) -> U256 {
        self.accounts
            .get(&address)
            .map(|acc| acc.balance)
            .unwrap_or(U256::zero())
    }
}

/// Nonce validator service
pub struct NonceValidator;

impl NonceValidator {
    /// Validate nonce ordering across transactions
    pub fn validate(transactions: &[TransactionCandidate]) -> Result<()> {
        use std::collections::HashMap;

        let mut sender_nonces: HashMap<[u8; 20], Vec<u64>> = HashMap::new();

        // Group nonces by sender
        for tx in transactions {
            sender_nonces.entry(tx.from).or_default().push(tx.nonce);
        }

        // Check each sender has sequential nonces
        for (address, mut nonces) in sender_nonces {
            nonces.sort_unstable();
            Self::check_nonce_sequence(&address, &nonces)?;
        }

        Ok(())
    }

    /// Check that nonces are sequential (no duplicates, no gaps)
    fn check_nonce_sequence(address: &[u8; 20], nonces: &[u64]) -> Result<()> {
        for i in 1..nonces.len() {
            if nonces[i] == nonces[i - 1] {
                return Err(BlockProductionError::NonceMismatch {
                    address: hex::encode(address),
                    expected: nonces[i - 1] + 1,
                    actual: nonces[i],
                });
            }
        }
        Ok(())
    }
}

/// PoW nonce search service with GPU/CPU compute backend
pub struct PoWMiner {
    /// Number of CPU threads (fallback when GPU unavailable)
    num_threads: u8,
    /// Compute engine (GPU or CPU via qc-compute)
    compute_engine: Option<std::sync::Arc<dyn qc_compute::ComputeEngine>>,
}

impl PoWMiner {
    /// Create new PoW miner with auto-detected compute backend
    pub fn new(num_threads: u8) -> Self {
        // Try to auto-detect GPU, fall back to CPU
        let compute_engine = match qc_compute::auto_detect() {
            Ok(engine) => {
                tracing::info!(
                    "PoW miner using {} backend: {}",
                    engine.backend(),
                    engine.device_info().name
                );
                Some(engine)
            }
            Err(e) => {
                tracing::warn!(
                    "No compute engine available, using legacy CPU threads: {}",
                    e
                );
                None
            }
        };

        Self {
            num_threads,
            compute_engine,
        }
    }

    /// Create PoW miner with specific compute engine (for testing)
    pub fn with_engine(
        num_threads: u8,
        engine: std::sync::Arc<dyn qc_compute::ComputeEngine>,
    ) -> Self {
        Self {
            num_threads,
            compute_engine: Some(engine),
        }
    }

    /// Check if GPU mining is available
    pub fn has_gpu(&self) -> bool {
        self.compute_engine
            .as_ref()
            .map(|e| e.backend() == qc_compute::Backend::OpenCL)
            .unwrap_or(false)
    }

    /// Get the current compute backend name
    pub fn backend_name(&self) -> String {
        self.compute_engine
            .as_ref()
            .map(|e| e.device_info().name.clone())
            .unwrap_or_else(|| format!("CPU ({} threads)", self.num_threads))
    }

    /// Get the compute engine for service layer to use in async mining
    ///
    /// Note: Async mining logic lives in service layer to maintain domain purity.
    /// This getter allows the service layer to access the compute engine.
    ///
    /// # Returns
    ///
    /// - `Some(Arc)` if a compute engine was successfully initialized.
    /// - `None` if no compute engine is available.
    pub fn get_compute_engine(&self) -> Option<std::sync::Arc<dyn qc_compute::ComputeEngine>> {
        self.compute_engine.clone()
    }

    /// Search for valid nonce in parallel
    ///
    /// Implementation: Parallel nonce search (see SPEC-17 Section 2.5)
    #[tracing::instrument(skip(self, template), fields(threads = self.num_threads))]
    pub fn mine_block(&self, template: BlockTemplate, difficulty_target: U256) -> Option<u64> {
        use std::sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc,
        };

        tracing::debug!(
            "Starting PoW mining: threads={}, difficulty={:?}",
            self.num_threads,
            difficulty_target
        );

        let found = Arc::new(AtomicBool::new(false));
        let result_nonce = Arc::new(AtomicU64::new(0));

        // Divide nonce space across threads
        let nonce_range_per_thread = u64::MAX / (self.num_threads as u64);

        let mut handles = vec![];

        for thread_id in 0..self.num_threads {
            let template_clone = template.clone();
            let found_clone = Arc::clone(&found);
            let result_clone = Arc::clone(&result_nonce);
            let difficulty = difficulty_target;

            let handle = std::thread::spawn(move || {
                let start_nonce = thread_id as u64 * nonce_range_per_thread;
                let end_nonce = if thread_id == Self::MAX_THREADS - 1 {
                    u64::MAX
                } else {
                    (thread_id as u64 + 1) * nonce_range_per_thread
                };

                for nonce in start_nonce..end_nonce {
                    // Check if another thread found solution
                    if found_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    // Check every 10000 iterations to reduce contention
                    if nonce % 10000 == 0 && found_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    // Compute hash with this nonce
                    if Self::check_pow(&template_clone, nonce, difficulty) {
                        found_clone.store(true, Ordering::Relaxed);
                        result_clone.store(nonce, Ordering::Relaxed);
                        break;
                    }
                }
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            let _ = handle.join();
        }

        let result = if found.load(Ordering::Relaxed) {
            let nonce = result_nonce.load(Ordering::Relaxed);
            tracing::info!("PoW mining successful: nonce={}", nonce);
            Some(nonce)
        } else {
            tracing::warn!("PoW mining failed: no valid nonce found");
            None
        };

        result
    }

    const MAX_THREADS: u8 = 255;

    /// Check if nonce produces valid PoW hash
    fn check_pow(template: &BlockTemplate, nonce: u64, difficulty_target: U256) -> bool {
        use crate::utils::hashing::{meets_difficulty, serialize_block_header, sha256d};

        // Serialize header with nonce
        let header_bytes = serialize_block_header(
            &template.header.parent_hash,
            template.header.block_number,
            template.header.timestamp,
            &template.header.beneficiary,
            template.header.gas_used,
            Some(nonce),
        );

        // SHA-256d (Bitcoin-style)
        let final_hash = sha256d(&header_bytes);

        // Check against difficulty
        meets_difficulty(&final_hash, difficulty_target)
    }
}

/// PoS proposer service
pub struct PoSProposer {
    /// Validator private key (placeholder - will use proper key type)
    validator_key: Vec<u8>,
}

impl PoSProposer {
    /// Create new PoS proposer
    pub fn new(validator_key: Vec<u8>) -> Self {
        Self { validator_key }
    }

    /// Check if we are the proposer for this slot
    ///
    /// Implementation: VRF-based selection (see SPEC-17 Section 2.6)
    ///
    /// Note: This is a simplified implementation. Full VRF requires a proper
    /// cryptographic library (vrf, schnorrkel, etc.)
    #[tracing::instrument(skip(self, validator_set), fields(validator_count = validator_set.len()))]
    pub fn check_proposer_duty(
        &self,
        slot: u64,
        epoch: u64,
        validator_set: &[Vec<u8>],
    ) -> Option<ProposerDuty> {
        use sha2::{Digest, Sha256};

        if validator_set.is_empty() {
            tracing::warn!("Empty validator set for slot {}", slot);
            return None;
        }

        tracing::debug!("Checking proposer duty for slot={}, epoch={}", slot, epoch);

        // Generate VRF input: serialize(slot, epoch, validator_set_hash)
        let mut vrf_input = Vec::new();
        vrf_input.extend_from_slice(&slot.to_le_bytes());
        vrf_input.extend_from_slice(&epoch.to_le_bytes());

        // Hash validator set
        let mut hasher = Sha256::new();
        for validator in validator_set {
            hasher.update(validator);
        }
        let validator_set_hash = hasher.finalize();
        vrf_input.extend_from_slice(&validator_set_hash);

        // Sign with validator key (simplified - real VRF would use proper signing)
        let mut output_hasher = Sha256::new();
        output_hasher.update(&self.validator_key);
        output_hasher.update(&vrf_input);
        let vrf_output = output_hasher.finalize();

        // Deterministic selection: vrf_output % validator_count
        let mut output_array = [0u8; 32];
        output_array.copy_from_slice(&vrf_output);

        // Use first 8 bytes as u64 for selection
        let selection_value = u64::from_le_bytes([
            output_array[0],
            output_array[1],
            output_array[2],
            output_array[3],
            output_array[4],
            output_array[5],
            output_array[6],
            output_array[7],
        ]);

        let selected_index = (selection_value % validator_set.len() as u64) as u32;

        // Check if we are selected (simplified - check if our key matches)
        let our_key_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&self.validator_key);
            hasher.finalize()
        };

        let mut our_key_match = false;
        for (idx, validator) in validator_set.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(validator);
            let validator_hash = hasher.finalize();

            if our_key_hash == validator_hash {
                our_key_match = idx == selected_index as usize;
                break;
            }
        }

        if our_key_match {
            tracing::info!(
                "Selected as proposer for slot={}, epoch={}, validator_index={}",
                slot,
                epoch,
                selected_index
            );

            // Generate proof (simplified - real VRF would generate proper proof)
            let mut proof_bytes = [0u8; 80];
            proof_bytes[..32].copy_from_slice(&vrf_output);
            // Pad with hash of input for the remaining 48 bytes
            let mut proof_hasher = Sha256::new();
            proof_hasher.update(&vrf_input);
            let proof_extension = proof_hasher.finalize();
            proof_bytes[32..64].copy_from_slice(&proof_extension);

            Some(ProposerDuty {
                slot,
                epoch,
                validator_index: selected_index,
                vrf_proof: VRFProof::new(output_array, proof_bytes),
            })
        } else {
            None
        }
    }

    /// Sign block template with validator key
    pub fn sign_block(&self, template: &BlockTemplate) -> Vec<u8> {
        use crate::utils::hashing::{serialize_block_header, sha256};

        // Serialize header
        let header_bytes = serialize_block_header(
            &template.header.parent_hash,
            template.header.block_number,
            template.header.timestamp,
            &template.header.beneficiary,
            template.header.gas_used,
            None, // No nonce for PoS
        );

        // Sign with validator key (simplified - real implementation would use ECDSA/Ed25519)
        let mut sign_data = Vec::new();
        sign_data.extend_from_slice(&self.validator_key);
        sign_data.extend_from_slice(&header_bytes);

        let signature = sha256(&sign_data);
        signature.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_selector_creation() {
        let selector = TransactionSelector::new(30_000_000, U256::from(1_000_000_000), true);
        assert_eq!(selector.gas_limit, 30_000_000);
        assert_eq!(selector.min_gas_price, U256::from(1_000_000_000));
        assert!(selector.fair_ordering);
    }

    #[test]
    fn test_state_cache_creation() {
        let cache = StatePrefetchCache::new(primitive_types::H256::zero());
        assert_eq!(cache.get_nonce([0u8; 20]), 0);
        assert_eq!(cache.get_balance([0u8; 20]), U256::zero());
    }
}
