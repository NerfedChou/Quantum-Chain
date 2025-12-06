# SPECIFICATION: BLOCK PRODUCTION ENGINE

**Version:** 2.4  
**Subsystem ID:** 17  
**Bounded Context:** Block Production & Mining  
**Crate Name:** `crates/qc-17-block-production`  
**Author:** Systems Architecture Team  
**Date:** 2025-12-06  
**Architecture Compliance:** Architecture.md v2.4, IPC-MATRIX.md v2.4, System.md v2.4

---

## 1. ABSTRACT

### 1.1 Purpose

The **Block Production Engine** subsystem is responsible for creating new blocks through intelligent transaction selection, state simulation, and consensus-appropriate sealing (PoW mining, PoS proposing, or PBFT leader proposal). This subsystem implements optimal algorithms for maximizing block profitability while maintaining security invariants.

### 1.2 Responsibility Boundaries

**In Scope:**
- Transaction selection using Priority-Based Greedy Knapsack algorithm
- Nonce dependency graph management for sequential transaction ordering
- State prefetch and simulation to avoid failed transactions
- PoW parallel nonce search across CPU threads
- PoS VRF-based proposer duty checking
- PBFT leader-based block proposal
- MEV (Miner Extractable Value) detection and fair ordering
- Block template creation and submission to Consensus

**Out of Scope:**
- Block validation (handled by Consensus - Subsystem 8)
- Block storage (handled by Block Storage - Subsystem 2)
- Transaction verification (handled by Signature Verification - Subsystem 10)
- Finality determination (handled by Finality - Subsystem 9)
- Smart contract execution (handled by Smart Contracts - Subsystem 11)

### 1.3 Key Design Principle: Optimal Transaction Selection

This subsystem solves the **bounded knapsack problem**:

```
Given:
  - Block gas limit: 30,000,000 gas (capacity)
  - Pending transactions: Each with gas_limit (weight) and gas_price (value)
  
Goal:
  - Maximize: Σ(gas_price × gas_used) 
  - Subject to: Σ(gas_used) ≤ block_gas_limit

Algorithm:
  - Priority-Based Greedy Knapsack
  - Complexity: O(n log n)
  - Maintains nonce dependency chains
```

This is NOT a simple fee-sorted list. The algorithm must:
1. Maintain per-sender nonce ordering (sequential)
2. Simulate execution to detect failures
3. Handle dynamic state changes during selection
4. Support MEV bundle ordering

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.4)                        │
├─────────────────────────────────────────────────────────────────┤
│  THIS SUBSYSTEM PRODUCES BLOCKS BUT DOES NOT VALIDATE THEM      │
│                                                                 │
│  INPUTS (Trusted):                                              │
│  ├─ Pending transactions from Mempool (6) - pre-verified       │
│  ├─ State from State Management (4) - authoritative            │
│  └─ Finality events from Finality (9) - triggers next block    │
│                                                                 │
│  OUTPUTS (Untrusted until validated):                           │
│  ├─ Block templates → Consensus (8) - MUST be validated        │
│  └─ Mining metrics → Telemetry - for observability             │
│                                                                 │
│  SECURITY PRINCIPLE:                                            │
│  - Consensus (8) re-validates ALL transactions                  │
│  - This subsystem can be Byzantine without breaking chain       │
│  - Worst case: Censorship (detectable) or empty blocks         │
│                                                                 │
│  INVARIANTS (Must Hold):                                        │
│  - sum(tx.gas_used) ≤ block_gas_limit                          │
│  - All transactions have sequential nonces per sender          │
│  - All transactions simulate successfully                       │
│  - No duplicate transaction hashes                             │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Block template created by this subsystem
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockTemplate {
    /// Block header (to be filled/signed)
    pub header: BlockHeader,
    
    /// Selected transactions in optimal order
    pub transactions: Vec<SignedTransaction>,
    
    /// Total gas used by all transactions
    pub total_gas_used: u64,
    
    /// Total fee revenue
    pub total_fees: U256,
    
    /// Consensus mode this block is for
    pub consensus_mode: ConsensusMode,
    
    /// Creation timestamp
    pub created_at: u64,
}

/// Block header (partially filled by this subsystem)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Parent block hash
    pub parent_hash: Hash,
    
    /// Block number (height)
    pub block_number: u64,
    
    /// Unix timestamp
    pub timestamp: u64,
    
    /// Beneficiary address (coinbase/validator)
    pub beneficiary: Address,
    
    /// Gas used in this block
    pub gas_used: u64,
    
    /// Gas limit for this block
    pub gas_limit: u64,
    
    /// Difficulty target (PoW only)
    pub difficulty: U256,
    
    /// Extra data (client ID, version)
    pub extra_data: Vec<u8>,
    
    /// Merkle root (filled by Transaction Indexing - Subsystem 3)
    pub merkle_root: Option<Hash>,
    
    /// State root (filled by State Management - Subsystem 4)
    pub state_root: Option<Hash>,
    
    /// Nonce (filled by PoW miner, omitted in PoS/PBFT)
    pub nonce: Option<u64>,
}

/// Consensus mode enumeration
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMode {
    /// Proof of Work - parallel nonce search
    ProofOfWork,
    
    /// Proof of Stake - VRF-based proposer selection
    ProofOfStake,
    
    /// PBFT - leader-based proposal
    PBFT,
}

/// Mining job for PoW mode
#[derive(Clone, Debug)]
pub struct MiningJob {
    /// Block template to mine
    pub template: BlockTemplate,
    
    /// Difficulty target
    pub difficulty_target: U256,
    
    /// Number of threads to use
    pub num_threads: u8,
    
    /// Nonce range per thread
    pub nonce_ranges: Vec<(u64, u64)>,
}

/// PoS proposer duty assignment
#[derive(Clone, Debug)]
pub struct ProposerDuty {
    /// Slot number
    pub slot: u64,
    
    /// Epoch number
    pub epoch: u64,
    
    /// Validator index in active set
    pub validator_index: u32,
    
    /// VRF proof of selection
    pub vrf_proof: VRFProof,
}

/// VRF proof for PoS proposer selection
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VRFProof {
    /// VRF output (32 bytes)
    pub output: [u8; 32],
    
    /// VRF proof (80 bytes)
    pub proof: [u8; 80],
}

/// Transaction with metadata for selection
#[derive(Clone, Debug)]
pub struct TransactionCandidate {
    /// The signed transaction
    pub transaction: SignedTransaction,
    
    /// Recovered sender address
    pub from: Address,
    
    /// Transaction nonce
    pub nonce: u64,
    
    /// Gas price (priority)
    pub gas_price: U256,
    
    /// Gas limit (maximum gas)
    pub gas_limit: u64,
    
    /// Pre-verified signature validity
    pub signature_valid: bool,
}

/// State simulation result
#[derive(Clone, Debug)]
pub struct SimulationResult {
    /// Transaction hash
    pub tx_hash: Hash,
    
    /// Simulation succeeded
    pub success: bool,
    
    /// Actual gas used (if success)
    pub gas_used: u64,
    
    /// State changes (for cache)
    pub state_changes: Vec<StateChange>,
    
    /// Error message (if failed)
    pub error: Option<String>,
}

/// State change from simulation
#[derive(Clone, Debug)]
pub struct StateChange {
    /// Address affected
    pub address: Address,
    
    /// Storage key (None for balance/nonce)
    pub storage_key: Option<Hash>,
    
    /// Old value
    pub old_value: Vec<u8>,
    
    /// New value
    pub new_value: Vec<u8>,
}

/// Transaction bundle for MEV
#[derive(Clone, Debug)]
pub struct TransactionBundle {
    /// Transactions in bundle (must be executed sequentially)
    pub transactions: Vec<SignedTransaction>,
    
    /// Bundle profitability
    pub profit: U256,
    
    /// Bundle type
    pub bundle_type: BundleType,
}

/// MEV bundle types
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BundleType {
    /// Simple bundle (user intent)
    Simple,
    
    /// Front-running bundle (detected MEV)
    FrontRunning,
    
    /// Back-running bundle (detected MEV)
    BackRunning,
    
    /// Sandwich attack (detected MEV)
    Sandwich,
}
```

### 2.2 Invariants

```rust
/// INVARIANT-1: Gas Limit Enforcement
/// The sum of all transaction gas_used MUST NOT exceed block_gas_limit.
#[invariant]
fn gas_limit_enforced(block: &BlockTemplate) -> bool {
    block.total_gas_used <= block.header.gas_limit
}

/// INVARIANT-2: Nonce Ordering
/// All transactions from the same sender MUST have sequential nonces.
#[invariant]
fn nonce_ordering(transactions: &[SignedTransaction]) -> bool {
    let mut nonces_by_sender: HashMap<Address, Vec<u64>> = HashMap::new();
    
    for tx in transactions {
        let sender = tx.from;
        nonces_by_sender.entry(sender).or_default().push(tx.nonce);
    }
    
    for (_, mut nonces) in nonces_by_sender {
        nonces.sort();
        for i in 1..nonces.len() {
            if nonces[i] != nonces[i-1] + 1 {
                return false; // Non-sequential
            }
        }
    }
    
    true
}

/// INVARIANT-3: State Validity
/// All included transactions MUST simulate successfully.
#[invariant]
fn all_transactions_valid(simulations: &[SimulationResult]) -> bool {
    simulations.iter().all(|sim| sim.success)
}

/// INVARIANT-4: No Duplicates
/// No transaction hash appears more than once.
#[invariant]
fn no_duplicate_transactions(transactions: &[SignedTransaction]) -> bool {
    let mut seen: HashSet<Hash> = HashSet::new();
    transactions.iter().all(|tx| seen.insert(tx.hash()))
}

/// INVARIANT-5: Timestamp Monotonicity
/// Block timestamp MUST be >= parent timestamp and <= current time + 15s.
#[invariant]
fn timestamp_valid(block: &BlockTemplate, parent_timestamp: u64, current_time: u64) -> bool {
    block.header.timestamp >= parent_timestamp
        && block.header.timestamp <= current_time + 15
}

/// INVARIANT-6: Fee Profitability
/// Selected transactions SHOULD be ordered by gas_price descending (greedy).
#[invariant]
fn fee_priority_ordering(transactions: &[TransactionCandidate]) -> bool {
    for i in 1..transactions.len() {
        if transactions[i].gas_price > transactions[i-1].gas_price {
            return false; // Higher gas price should come first
        }
    }
    true
}
```

### 2.3 Domain Services

```rust
/// Transaction selector service (core domain logic)
pub struct TransactionSelector {
    /// Block gas limit
    gas_limit: u64,
    
    /// Minimum gas price threshold
    min_gas_price: U256,
    
    /// MEV protection enabled
    fair_ordering: bool,
}

impl TransactionSelector {
    /// Select optimal transaction set using greedy knapsack
    pub fn select_transactions(
        &self,
        candidates: Vec<TransactionCandidate>,
        state_cache: &StatePrefetchCache,
    ) -> TransactionSelectionResult {
        // Algorithm implementation (see Section 2.4)
    }
    
    /// Validate nonce ordering for a set of transactions
    pub fn validate_nonce_ordering(
        &self,
        transactions: &[TransactionCandidate],
    ) -> Result<(), NonceMismatch> {
        // Validation logic
    }
    
    /// Detect MEV bundles in transaction set
    pub fn detect_mev_bundles(
        &self,
        transactions: &[SignedTransaction],
    ) -> Vec<TransactionBundle> {
        // MEV detection heuristics
    }
}

/// State prefetch cache for simulation
pub struct StatePrefetchCache {
    /// Parent state root
    parent_state_root: Hash,
    
    /// Cached account states
    accounts: HashMap<Address, AccountState>,
    
    /// Cached storage slots
    storage: HashMap<(Address, Hash), Vec<u8>>,
}

impl StatePrefetchCache {
    /// Simulate transaction execution
    pub fn simulate_transaction(
        &mut self,
        tx: &SignedTransaction,
    ) -> SimulationResult {
        // Simulation logic
    }
    
    /// Apply simulation result to cache
    pub fn apply_state_changes(&mut self, changes: &[StateChange]) {
        // Update cache
    }
    
    /// Get current nonce for address
    pub fn get_nonce(&self, address: Address) -> u64 {
        self.accounts.get(&address).map(|acc| acc.nonce).unwrap_or(0)
    }
}

/// PoW nonce search service
pub struct PoWMiner {
    /// Number of threads
    num_threads: u8,
}

impl PoWMiner {
    /// Search for valid nonce in parallel
    pub fn mine_block(
        &self,
        template: BlockTemplate,
        difficulty_target: U256,
    ) -> Option<u64> {
        // Parallel mining implementation (see Section 2.5)
    }
}

/// PoS proposer service
pub struct PoSProposer {
    /// Validator private key
    validator_key: SecretKey,
}

impl PoSProposer {
    /// Check if we are the proposer for this slot
    pub fn check_proposer_duty(
        &self,
        slot: u64,
        epoch: u64,
        validator_set: &[PublicKey],
    ) -> Option<ProposerDuty> {
        // VRF-based proposer selection (see Section 2.6)
    }
    
    /// Sign block template with validator key
    pub fn sign_block(&self, template: &BlockTemplate) -> Signature {
        // Block signing logic
    }
}
```

### 2.4 Algorithm: Transaction Selection (Greedy Knapsack)

```rust
/// Transaction selection using Priority-Based Greedy Knapsack
/// 
/// Complexity: O(n log n)
/// Memory: O(n)
pub fn select_transactions(
    candidates: Vec<TransactionCandidate>,
    block_gas_limit: u64,
    state_cache: &mut StatePrefetchCache,
) -> Vec<SignedTransaction> {
    // STEP 1: Group transactions by sender (O(n))
    let mut tx_groups: HashMap<Address, Vec<TransactionCandidate>> = HashMap::new();
    for tx in candidates {
        tx_groups.entry(tx.from).or_default().push(tx);
    }
    
    // STEP 2: Sort each group by nonce ascending (O(n log n) total)
    for (_, txs) in tx_groups.iter_mut() {
        txs.sort_by_key(|tx| tx.nonce);
    }
    
    // STEP 3: Build priority queue with first valid tx from each sender (O(n log n))
    let mut priority_queue = BinaryHeap::new();
    for (sender, txs) in &tx_groups {
        if let Some(first_tx) = txs.first() {
            let expected_nonce = state_cache.get_nonce(*sender);
            if first_tx.nonce == expected_nonce {
                // Priority = negative gas_price for max-heap behavior
                priority_queue.push(Reverse((first_tx.gas_price, first_tx.clone())));
            }
        }
    }
    
    // STEP 4: Greedy selection (knapsack) (O(n log n))
    let mut selected = Vec::new();
    let mut total_gas = 0u64;
    let mut sender_indices: HashMap<Address, usize> = HashMap::new();
    
    while let Some(Reverse((_, tx))) = priority_queue.pop() {
        // Check gas limit
        if total_gas + tx.gas_limit > block_gas_limit {
            continue; // Skip, doesn't fit
        }
        
        // Simulate execution
        let sim_result = state_cache.simulate_transaction(&tx.transaction);
        
        if sim_result.success {
            // Include transaction
            selected.push(tx.transaction.clone());
            total_gas += sim_result.gas_used;
            
            // Apply state changes to cache
            state_cache.apply_state_changes(&sim_result.state_changes);
            
            // Add next transaction from same sender (if exists)
            let sender = tx.from;
            let next_index = sender_indices.get(&sender).copied().unwrap_or(0) + 1;
            
            if let Some(sender_txs) = tx_groups.get(&sender) {
                if next_index < sender_txs.len() {
                    let next_tx = &sender_txs[next_index];
                    let expected_nonce = state_cache.get_nonce(sender);
                    
                    if next_tx.nonce == expected_nonce {
                        priority_queue.push(Reverse((next_tx.gas_price, next_tx.clone())));
                        sender_indices.insert(sender, next_index);
                    }
                }
            }
        }
        // If simulation failed, skip transaction (don't include)
    }
    
    selected
}
```

**Algorithm Properties:**
- **Complexity:** O(n log n) where n = number of pending transactions
- **Optimality:** Greedy approximation (near-optimal for this NP-hard problem)
- **Correctness:** Maintains nonce ordering invariant
- **Safety:** Only includes transactions that simulate successfully

### 2.5 Algorithm: Parallel PoW Mining

```rust
/// Parallel nonce search across CPU threads
/// 
/// Complexity: O(2^64 / num_threads) expected nonce checks
/// Parallelization: Linear speedup with thread count
pub fn mine_block_pow(
    template: BlockTemplate,
    difficulty_target: U256,
    num_threads: u8,
) -> Option<u64> {
    let nonce_space = u64::MAX;
    let chunk_size = nonce_space / (num_threads as u64);
    
    // Spawn worker threads
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::new();
    
    for thread_id in 0..num_threads {
        let template = template.clone();
        let tx = tx.clone();
        let start_nonce = (thread_id as u64) * chunk_size;
        let end_nonce = if thread_id == num_threads - 1 {
            u64::MAX
        } else {
            start_nonce + chunk_size
        };
        
        let handle = thread::spawn(move || {
            search_nonce_range(template, difficulty_target, start_nonce, end_nonce, tx)
        });
        
        handles.push(handle);
    }
    
    // Wait for first valid nonce
    drop(tx); // Close sender to allow rx.recv() to eventually return Err
    if let Ok(nonce) = rx.recv() {
        // Kill all threads (they will exit when checking atomic flag)
        for handle in handles {
            let _ = handle.join();
        }
        Some(nonce)
    } else {
        None // No valid nonce found (extremely unlikely)
    }
}

fn search_nonce_range(
    mut template: BlockTemplate,
    difficulty_target: U256,
    start: u64,
    end: u64,
    tx: mpsc::Sender<u64>,
) {
    for nonce in start..end {
        template.header.nonce = Some(nonce);
        
        // Serialize header
        let header_bytes = serialize_block_header(&template.header);
        
        // Double SHA-256 (Bitcoin-style) or Keccak-256 (Ethereum-style)
        let hash = sha256d(&header_bytes);
        let hash_value = U256::from_big_endian(&hash);
        
        // Check if hash meets difficulty target
        if hash_value <= difficulty_target {
            // Found valid nonce!
            let _ = tx.send(nonce);
            return;
        }
        
        // Check if another thread found solution (early exit)
        if tx.is_closed() {
            return;
        }
    }
}
```

### 2.6 Algorithm: VRF Proposer Selection (PoS)

```rust
/// VRF-based proposer duty checking
/// 
/// Properties:
/// - Unpredictable: Attacker cannot predict future proposers
/// - Verifiable: Anyone can verify VRF proof
/// - Unbiasable: Proposer cannot manipulate selection
pub fn check_proposer_duty(
    validator_key: &SecretKey,
    slot: u64,
    epoch: u64,
    validator_set: &[PublicKey],
) -> Option<ProposerDuty> {
    // STEP 1: Generate VRF input
    let vrf_input = serialize_vrf_input(slot, epoch, hash_validator_set(validator_set));
    
    // STEP 2: Sign with validator private key
    let (vrf_output, vrf_proof) = vrf_sign(validator_key, &vrf_input);
    
    // STEP 3: Deterministic selection based on VRF output
    let selected_index = vrf_output_to_index(&vrf_output, validator_set.len());
    let selected_validator = validator_set[selected_index];
    
    // STEP 4: Check if we are selected
    let our_pubkey = validator_key.public_key();
    if selected_validator == our_pubkey {
        Some(ProposerDuty {
            slot,
            epoch,
            validator_index: selected_index as u32,
            vrf_proof: VRFProof {
                output: vrf_output,
                proof: vrf_proof,
            },
        })
    } else {
        None // Not our turn
    }
}

fn vrf_output_to_index(vrf_output: &[u8; 32], set_size: usize) -> usize {
    let value = u64::from_be_bytes([
        vrf_output[0], vrf_output[1], vrf_output[2], vrf_output[3],
        vrf_output[4], vrf_output[5], vrf_output[6], vrf_output[7],
    ]);
    (value % (set_size as u64)) as usize
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Inbound Ports (Driving Side - API)

```rust
/// Primary port: Block production service
#[async_trait]
pub trait BlockProducerService: Send + Sync {
    /// Produce a new block
    async fn produce_block(
        &self,
        parent_hash: Hash,
        beneficiary: Address,
    ) -> Result<BlockTemplate, BlockProductionError>;
    
    /// Start mining/proposing
    async fn start_production(
        &self,
        mode: ConsensusMode,
        config: ProductionConfig,
    ) -> Result<(), BlockProductionError>;
    
    /// Stop mining/proposing
    async fn stop_production(&self) -> Result<(), BlockProductionError>;
    
    /// Get current mining/proposing status
    async fn get_status(&self) -> ProductionStatus;
    
    /// Update block gas limit
    async fn update_gas_limit(&self, new_limit: u64) -> Result<(), BlockProductionError>;
    
    /// Update minimum gas price
    async fn update_min_gas_price(&self, new_price: U256) -> Result<(), BlockProductionError>;
}

#[derive(Clone, Debug)]
pub struct ProductionConfig {
    /// Consensus mode
    pub mode: ConsensusMode,
    
    /// Number of threads (PoW only)
    pub pow_threads: Option<u8>,
    
    /// Validator key (PoS only)
    pub validator_key: Option<SecretKey>,
    
    /// Block gas limit
    pub gas_limit: u64,
    
    /// Minimum gas price
    pub min_gas_price: U256,
    
    /// Enable MEV protection
    pub fair_ordering: bool,
}

#[derive(Clone, Debug)]
pub struct ProductionStatus {
    /// Is currently producing blocks
    pub active: bool,
    
    /// Current consensus mode
    pub mode: Option<ConsensusMode>,
    
    /// Blocks produced this session
    pub blocks_produced: u64,
    
    /// Total fees collected
    pub total_fees: U256,
    
    /// Current hashrate (PoW only)
    pub hashrate: Option<f64>,
    
    /// Last block produced timestamp
    pub last_block_at: Option<u64>,
}
```

### 3.2 Outbound Ports (Driven Side - SPI)

```rust
/// Port: Fetch pending transactions from Mempool
#[async_trait]
pub trait MempoolReader: Send + Sync {
    /// Get pending transactions
    async fn get_pending_transactions(
        &self,
        max_count: u32,
        min_gas_price: U256,
    ) -> Result<Vec<TransactionCandidate>, MempoolError>;
    
    /// Get mempool status
    async fn get_mempool_status(&self) -> Result<MempoolStatus, MempoolError>;
}

/// Port: State prefetch and simulation
#[async_trait]
pub trait StateReader: Send + Sync {
    /// Create state prefetch cache
    async fn create_prefetch_cache(
        &self,
        state_root: Hash,
    ) -> Result<StatePrefetchCache, StateError>;
    
    /// Simulate transaction batch
    async fn simulate_transactions(
        &self,
        state_root: Hash,
        transactions: Vec<SignedTransaction>,
    ) -> Result<Vec<SimulationResult>, StateError>;
}

/// Port: Submit produced block to Consensus
#[async_trait]
pub trait ConsensusSubmitter: Send + Sync {
    /// Submit block template for validation
    async fn submit_block(
        &self,
        template: BlockTemplate,
        consensus_proof: ConsensusProof,
    ) -> Result<SubmissionReceipt, ConsensusError>;
}

#[derive(Clone, Debug)]
pub struct ConsensusProof {
    /// PoW nonce (if applicable)
    pub pow_nonce: Option<u64>,
    
    /// PoS VRF proof (if applicable)
    pub pos_vrf_proof: Option<VRFProof>,
    
    /// PoS validator signature (if applicable)
    pub pos_signature: Option<Signature>,
    
    /// PBFT leader signature (if applicable)
    pub pbft_signature: Option<Signature>,
}

/// Port: Sign blocks with validator key
#[async_trait]
pub trait SignatureProvider: Send + Sync {
    /// Sign block header
    async fn sign_block_header(
        &self,
        header: &BlockHeader,
        key: &SecretKey,
    ) -> Result<Signature, SignatureError>;
    
    /// Verify VRF proof
    async fn verify_vrf_proof(
        &self,
        proof: &VRFProof,
        input: &[u8],
        public_key: &PublicKey,
    ) -> Result<bool, SignatureError>;
}

/// Port: Publish events to Event Bus
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish BlockProduced event
    async fn publish_block_produced(
        &self,
        event: BlockProducedEvent,
    ) -> Result<(), EventError>;
    
    /// Publish MiningMetrics event
    async fn publish_mining_metrics(
        &self,
        metrics: MiningMetrics,
    ) -> Result<(), EventError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Subscribed Events (Inbound)

```rust
/// Event from Finality (9): Block finalized, produce next block
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockFinalizedEvent {
    pub version: u16,
    pub sender_id: SubsystemId,  // Must be 9
    pub block_hash: Hash,
    pub block_number: u64,
    pub finalized_at: u64,
}

/// Event from Consensus (8): PoS proposer duty assigned
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotAssignedEvent {
    pub version: u16,
    pub sender_id: SubsystemId,  // Must be 8
    pub slot: u64,
    pub epoch: u64,
    pub validator_index: u32,
    pub vrf_proof: VRFProof,
}

/// Event from Mempool (6): New transaction added (optional optimization)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewPendingTransactionEvent {
    pub version: u16,
    pub sender_id: SubsystemId,  // Must be 6
    pub tx_hash: Hash,
    pub gas_price: U256,
    pub gas_limit: u64,
}
```

### 4.2 Published Events (Outbound)

```rust
/// Event: Block successfully produced
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockProducedEvent {
    pub version: u16,
    pub sender_id: SubsystemId,  // Always 17
    pub block_hash: Hash,
    pub block_number: u64,
    pub transaction_count: u32,
    pub total_gas_used: u64,
    pub total_fees: U256,
    pub production_time_ms: u64,
    pub consensus_mode: ConsensusMode,
    pub timestamp: u64,
}

/// Event: Mining/proposing metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiningMetrics {
    pub version: u16,
    pub sender_id: SubsystemId,  // Always 17
    
    // Transaction selection metrics
    pub transactions_considered: u32,
    pub transactions_selected: u32,
    pub total_gas_used: u64,
    pub total_fees: U256,
    pub selection_time_ms: u64,
    
    // PoW specific
    pub hashrate: Option<f64>,
    pub mining_time_ms: Option<u64>,
    
    // PoS specific
    pub slot_number: Option<u64>,
    
    // MEV metrics
    pub mev_bundles_detected: u32,
    pub mev_profit: U256,
    
    pub timestamp: u64,
}
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Phase 1: RED (Write Failing Tests)

#### Unit Tests (Domain Layer)

```rust
#[cfg(test)]
mod transaction_selection_tests {
    #[test]
    fn test_greedy_selection_by_gas_price() {
        // Setup: 3 transactions with different gas prices
        let txs = vec![
            make_tx(gas_price: 100, gas_limit: 1000),
            make_tx(gas_price: 200, gas_limit: 1000),
            make_tx(gas_price: 150, gas_limit: 1000),
        ];
        
        let selected = select_transactions(txs, gas_limit: 3000);
        
        // Assert: Higher gas price selected first
        assert_eq!(selected[0].gas_price, 200);
        assert_eq!(selected[1].gas_price, 150);
        assert_eq!(selected[2].gas_price, 100);
    }
    
    #[test]
    fn test_nonce_ordering_enforced() {
        // Setup: Transactions with non-sequential nonces
        let txs = vec![
            make_tx(from: alice, nonce: 0, gas_price: 100),
            make_tx(from: alice, nonce: 2, gas_price: 100), // Gap!
        ];
        
        let selected = select_transactions(txs, gas_limit: 10000);
        
        // Assert: Only nonce 0 selected (nonce 2 skipped)
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].nonce, 0);
    }
    
    #[test]
    fn test_gas_limit_enforcement() {
        // Setup: Transactions totaling more than gas limit
        let txs = vec![
            make_tx(gas_price: 100, gas_limit: 20_000_000),
            make_tx(gas_price: 90, gas_limit: 15_000_000),
        ];
        
        let selected = select_transactions(txs, gas_limit: 30_000_000);
        
        // Assert: Only first tx selected (second doesn't fit)
        assert_eq!(selected.len(), 1);
        assert!(selected[0].gas_limit == 20_000_000);
    }
    
    #[test]
    fn test_state_simulation_filters_failures() {
        // Setup: One valid tx, one that will fail
        let txs = vec![
            make_valid_tx(),
            make_tx_that_will_fail(), // Insufficient balance
        ];
        
        let selected = select_transactions(txs, gas_limit: 30_000_000);
        
        // Assert: Only valid tx included
        assert_eq!(selected.len(), 1);
        assert!(selected[0].hash() == make_valid_tx().hash());
    }
}

#[cfg(test)]
mod pow_mining_tests {
    #[test]
    fn test_parallel_mining_finds_nonce() {
        let template = make_test_template();
        let difficulty = U256::from(0x00000000ffff0000u64);
        
        let nonce = mine_block_pow(template, difficulty, threads: 4);
        
        assert!(nonce.is_some());
        // Verify nonce produces valid hash
        let hash = compute_hash_with_nonce(template, nonce.unwrap());
        assert!(U256::from_big_endian(&hash) <= difficulty);
    }
    
    #[test]
    fn test_mining_respects_difficulty() {
        let template = make_test_template();
        let easy_difficulty = U256::MAX; // Very easy
        
        let nonce = mine_block_pow(template, easy_difficulty, threads: 1);
        
        // Should find nonce very quickly
        assert!(nonce.is_some());
    }
}

#[cfg(test)]
mod pos_proposer_tests {
    #[test]
    fn test_vrf_proposer_selection_deterministic() {
        let validator_key = make_test_key();
        let validator_set = make_test_validator_set(10);
        
        // Same inputs should produce same result
        let duty1 = check_proposer_duty(&validator_key, slot: 100, epoch: 5, &validator_set);
        let duty2 = check_proposer_duty(&validator_key, slot: 100, epoch: 5, &validator_set);
        
        assert_eq!(duty1.is_some(), duty2.is_some());
    }
    
    #[test]
    fn test_vrf_proof_verifiable() {
        let validator_key = make_test_key();
        let validator_set = make_test_validator_set(10);
        
        if let Some(duty) = check_proposer_duty(&validator_key, slot: 100, epoch: 5, &validator_set) {
            // Anyone should be able to verify the VRF proof
            let valid = verify_vrf_proof(
                &duty.vrf_proof,
                &vrf_input(100, 5, validator_set),
                &validator_key.public_key(),
            );
            assert!(valid);
        }
    }
}
```

#### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_block_production_pow() {
    // Setup: Mock subsystems
    let mempool = MockMempool::with_transactions(vec![tx1, tx2, tx3]);
    let state = MockState::with_initial_state(genesis_state);
    let consensus = MockConsensus::new();
    
    // Create block producer
    let producer = BlockProducer::new(mempool, state, consensus);
    
    // Start PoW mining
    producer.start_production(ConsensusMode::ProofOfWork, config).await.unwrap();
    
    // Wait for block production
    sleep(Duration::from_secs(5)).await;
    
    // Verify block submitted to consensus
    let submitted_blocks = consensus.get_submitted_blocks();
    assert_eq!(submitted_blocks.len(), 1);
    assert!(submitted_blocks[0].header.nonce.is_some());
}

#[tokio::test]
async fn test_end_to_end_block_production_pos() {
    // Setup
    let mempool = MockMempool::with_transactions(vec![tx1, tx2, tx3]);
    let state = MockState::with_initial_state(genesis_state);
    let consensus = MockConsensus::new();
    
    let producer = BlockProducer::new(mempool, state, consensus);
    
    // Configure as PoS validator
    let validator_key = load_test_validator_key();
    producer.start_production(ConsensusMode::ProofOfStake, config_with_key(validator_key)).await.unwrap();
    
    // Simulate slot assignment
    producer.handle_slot_assigned(SlotAssignedEvent { slot: 10, ... }).await;
    
    // Verify block produced with VRF proof
    let submitted_blocks = consensus.get_submitted_blocks();
    assert_eq!(submitted_blocks.len(), 1);
    assert!(submitted_blocks[0].consensus_proof.pos_vrf_proof.is_some());
}
```

### 5.2 Phase 2: GREEN (Minimal Implementation)

Implement just enough to pass tests:
1. Transaction selection algorithm (greedy knapsack)
2. Nonce ordering validation
3. State simulation (mock initially)
4. Parallel PoW mining
5. VRF proposer selection

### 5.3 Phase 3: REFACTOR

Extract common patterns:
- Transaction selector as separate service
- State prefetch cache abstraction
- Mining/proposing strategy pattern
- Clean up error handling

---

## 6. ERROR HANDLING

### 6.1 Error Types

```rust
#[derive(Debug, Error)]
pub enum BlockProductionError {
    #[error("Mempool error: {0}")]
    MempoolError(#[from] MempoolError),
    
    #[error("State error: {0}")]
    StateError(#[from] StateError),
    
    #[error("Consensus error: {0}")]
    ConsensusError(#[from] ConsensusError),
    
    #[error("No transactions available")]
    NoTransactionsAvailable,
    
    #[error("Gas limit exceeded: used {used}, limit {limit}")]
    GasLimitExceeded { used: u64, limit: u64 },
    
    #[error("Nonce mismatch for {address}: expected {expected}, got {actual}")]
    NonceMismatch {
        address: Address,
        expected: u64,
        actual: u64,
    },
    
    #[error("Mining failed: no valid nonce found")]
    MiningFailed,
    
    #[error("Not selected as proposer for slot {slot}")]
    NotProposer { slot: u64 },
    
    #[error("Invalid validator key")]
    InvalidValidatorKey,
    
    #[error("Production not active")]
    NotActive,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}
```

### 6.2 Error Recovery Strategy

| Error | Recovery Action | Retry? |
|-------|----------------|--------|
| `NoTransactionsAvailable` | Produce empty block (if allowed) or wait | Yes, after delay |
| `GasLimitExceeded` | Internal logic error - should not happen | No, log and alert |
| `NonceMismatch` | Refetch state and retry selection | Yes, once |
| `MiningFailed` | Increase difficulty or wait | Yes, continuously |
| `NotProposer` | Wait for next slot assignment | Yes, next slot |
| `MempoolError` | Log error, wait for mempool recovery | Yes, with backoff |
| `StateError` | Log error, wait for state recovery | Yes, with backoff |

---

## 7. CONFIGURATION

### 7.1 Configuration Schema

```toml
[block_production]
# Consensus mode: "pow", "pos", or "pbft"
mode = "pos"

# Block gas limit (default: 30,000,000)
gas_limit = 30_000_000

# Minimum gas price in gwei (default: 1)
min_gas_price = 1

# Enable MEV protection (fair ordering)
fair_ordering = true

# Minimum transactions per block (0 = allow empty blocks)
min_transactions = 1

# PoW specific settings
[block_production.pow]
# Number of mining threads (default: num_cpus)
threads = 8

# Hash algorithm: "sha256d" (Bitcoin) or "keccak256" (Ethereum)
algorithm = "keccak256"

# PoS specific settings
[block_production.pos]
# Path to validator private key
validator_key_path = "/keys/validator.key"

# Slot duration in seconds (default: 12)
slot_duration = 12

# PBFT specific settings
[block_production.pbft]
# Validator ID in the validator set
validator_id = 3

# Total number of validators
total_validators = 10

# View change timeout in seconds
view_change_timeout = 30

# Performance tuning
[block_production.performance]
# Max transactions to consider (default: 10000)
max_transaction_candidates = 10_000

# State prefetch cache size in MB (default: 256)
prefetch_cache_size_mb = 256

# Enable parallel simulation (experimental)
parallel_simulation = false
```

### 7.2 Runtime Configuration

```rust
#[derive(Clone, Debug, Deserialize)]
pub struct BlockProductionConfig {
    pub mode: ConsensusMode,
    pub gas_limit: u64,
    pub min_gas_price: U256,
    pub fair_ordering: bool,
    pub min_transactions: u32,
    pub pow: Option<PoWConfig>,
    pub pos: Option<PoSConfig>,
    pub pbft: Option<PBFTConfig>,
    pub performance: PerformanceConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PoWConfig {
    pub threads: u8,
    pub algorithm: HashAlgorithm,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PoSConfig {
    pub validator_key_path: PathBuf,
    pub slot_duration: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PBFTConfig {
    pub validator_id: u32,
    pub total_validators: u32,
    pub view_change_timeout: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PerformanceConfig {
    pub max_transaction_candidates: u32,
    pub prefetch_cache_size_mb: u64,
    pub parallel_simulation: bool,
}
```

---

## APPENDIX A: DEPENDENCY MATRIX

### A.1 Depends On

| Subsystem | Relationship | Message Types | Critical For |
|-----------|-------------|---------------|-------------|
| 6 (Mempool) | Transaction source | `GetPendingTransactionsRequest` | Transaction selection |
| 4 (State Management) | State simulation | `StatePrefetchRequest` | Validity checking |
| 8 (Consensus) | Block submission | `ProduceBlockRequest` | Block validation |
| 10 (Signature Verification) | Key signing (PoS) | `SignBlockRequest` | PoS proposing |
| 9 (Finality) | Trigger events | `BlockFinalizedEvent` | Next block trigger |

### A.2 Provides To

| Subsystem | Relationship | Message Types | Critical For |
|-----------|-------------|---------------|-------------|
| 8 (Consensus) | Block templates | `ProduceBlockRequest` | Block validation |
| Event Bus | Metrics | `BlockProducedEvent`, `MiningMetrics` | Observability |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

### B.1 Allowed Senders

✅ Subsystem 6 (Mempool) - Transaction data  
✅ Subsystem 4 (State Management) - State prefetch responses  
✅ Subsystem 9 (Finality) - Block finalized events  
✅ Subsystem 8 (Consensus) - Slot assigned events  
✅ Admin CLI (localhost) - Start/stop commands  

❌ All other subsystems  
❌ External network sources  

### B.2 Allowed Recipients

✅ Subsystem 6 (Mempool) - Transaction queries  
✅ Subsystem 4 (State Management) - State queries  
✅ Subsystem 8 (Consensus) - Block submission  
✅ Subsystem 10 (Signature Verification) - Key signing  
✅ Event Bus - Metrics and events  

❌ All other subsystems  

### B.3 Attack Scenarios

**Scenario 1: Compromised Block Producer**
- **Attack:** Produce blocks with invalid transactions
- **Defense:** Consensus (8) re-validates all transactions
- **Result:** Invalid blocks rejected, no chain corruption

**Scenario 2: Transaction Censorship**
- **Attack:** Deliberately exclude specific transactions
- **Defense:** Cryptographic inclusion proofs, community detection
- **Result:** Censorship detectable but not preventable (design trade-off)

**Scenario 3: MEV Exploitation**
- **Attack:** Reorder transactions for front-running profit
- **Defense:** Fair ordering enforcement, MEV metrics transparency
- **Result:** Limited MEV possible within gas price tiers

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Architecture Documents

| Document | Section | Reference |
|----------|---------|-----------|
| Architecture.md | Section 5.1 | Choreography pattern (event-driven) |
| IPC-MATRIX.md | Subsystem 17 | Message types, security boundaries |
| System.md | Subsystem 17 | Algorithms, dependencies, security |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-06-MEMPOOL.md | Provides | Transaction source |
| SPEC-04-STATE-MANAGEMENT.md | Provides | State prefetch |
| SPEC-08-CONSENSUS.md | Consumes | Block validation |
| SPEC-09-FINALITY.md | Triggers | Block production |
| SPEC-10-SIGNATURE-VERIFICATION.md | Provides | Key signing (PoS) |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 5 (Block Production - Weeks 15-16)** because:
- Depends on Mempool (6), State Management (4), Consensus (8), Finality (9)
- Enables self-sufficient block creation
- Required for testnet/mainnet launch
- Can be developed in parallel with advanced subsystems (11-15)

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (greedy knapsack algorithm)
4. Implement PoW/PoS/PBFT adapters
5. Wire to node-runtime
6. Performance benchmarking and optimization
