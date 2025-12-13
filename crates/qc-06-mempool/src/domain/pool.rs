//! # Transaction Pool - Priority Queue and Two-Phase Commit
//!
//! Implements the core mempool data structure per SPEC-06 Section 2.1.
//!
//! ## Data Structures
//!
//! - `by_hash`: O(1) lookup by transaction hash
//! - `by_price`: O(log n) priority queue (BTreeSet)
//! - `by_sender`: O(log n) nonce-ordered transactions per account
//!
//! ## Invariants Enforced
//!
//! - INVARIANT-1: No duplicate hashes (checked in `add()`)
//! - INVARIANT-2: Nonce ordering per sender (BTreeMap keys)
//! - INVARIANT-3: PENDING_INCLUSION excluded from proposals (`by_price` only has PENDING)
//! - INVARIANT-5: Auto-rollback on timeout (`cleanup_timeouts()`)

use super::entities::{
    Address, Hash, MempoolConfig, MempoolTransaction, Timestamp, TransactionState, U256,
};
use super::errors::MempoolError;
use super::value_objects::{
    MempoolStatus, PendingInclusionBatch, PricedTransaction, ProposeResult,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Transaction priority queue with multiple indices.
///
/// Provides O(log n) operations for:
/// - Adding transactions
/// - Getting highest priority transactions
/// - Finding transactions by hash
/// - Finding transactions by sender
///
/// INVARIANTS:
/// - INVARIANT-1: No duplicate transaction hashes
/// - INVARIANT-2: Transactions from same sender ordered by nonce
/// - INVARIANT-3: PendingInclusion transactions excluded from get_for_block
/// - INVARIANT-5: Timed out PendingInclusion transactions auto-rollback
#[derive(Debug)]
pub struct TransactionPool {
    /// Configuration.
    config: MempoolConfig,

    /// All transactions indexed by hash.
    by_hash: HashMap<Hash, MempoolTransaction>,

    /// Transactions ordered by gas price (for priority selection).
    /// Only contains PENDING transactions (not pending_inclusion).
    by_price: BTreeSet<PricedTransaction>,

    /// Transactions grouped by sender, ordered by nonce.
    by_sender: HashMap<Address, BTreeMap<u64, Hash>>,

    /// Pending inclusion batches for tracking.
    pending_batches: Vec<PendingInclusionBatch>,
}

impl TransactionPool {
    /// Creates a new empty transaction pool.
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            config,
            by_hash: HashMap::new(),
            by_price: BTreeSet::new(),
            by_sender: HashMap::new(),
            pending_batches: Vec::new(),
        }
    }

    /// Creates a pool with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(MempoolConfig::default())
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &MempoolConfig {
        &self.config
    }

    /// Returns the number of transactions in the pool.
    pub fn len(&self) -> usize {
        self.by_hash.len()
    }

    /// Returns true if the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.by_hash.is_empty()
    }

    /// Returns the number of pending (not pending_inclusion) transactions.
    pub fn pending_count(&self) -> usize {
        self.by_price.len()
    }

    /// Returns the number of transactions pending inclusion.
    pub fn pending_inclusion_count(&self) -> usize {
        self.by_hash.len() - self.by_price.len()
    }

    /// Gets a transaction by hash.
    pub fn get(&self, hash: &Hash) -> Option<&MempoolTransaction> {
        self.by_hash.get(hash)
    }

    /// Gets a mutable transaction by hash.
    pub fn get_mut(&mut self, hash: &Hash) -> Option<&mut MempoolTransaction> {
        self.by_hash.get_mut(hash)
    }

    /// Checks if a transaction exists in the pool.
    pub fn contains(&self, hash: &Hash) -> bool {
        self.by_hash.contains_key(hash)
    }

    /// Gets the state of a transaction.
    pub fn get_state(&self, hash: &Hash) -> Option<TransactionState> {
        self.by_hash.get(hash).map(|tx| tx.state)
    }

    /// Adds a transaction to the pool.
    ///
    /// # Errors
    /// - `DuplicateTransaction` if hash already exists
    /// - `GasPriceTooLow` if below minimum
    /// - `GasLimitTooHigh` if above maximum
    /// - `AccountLimitReached` if sender has too many transactions
    /// - `PoolFull` if at capacity (and new tx doesn't qualify for eviction)
    pub fn add(&mut self, tx: MempoolTransaction) -> Result<(), MempoolError> {
        // Check for duplicate
        if self.by_hash.contains_key(&tx.hash) {
            return Err(MempoolError::DuplicateTransaction(tx.hash));
        }

        // Validate gas price
        if tx.gas_price < self.config.min_gas_price {
            return Err(MempoolError::GasPriceTooLow {
                price: tx.gas_price,
                minimum: self.config.min_gas_price,
            });
        }

        // Validate gas limit
        if tx.gas_limit > self.config.max_gas_per_tx {
            return Err(MempoolError::GasLimitTooHigh {
                limit: tx.gas_limit,
                maximum: self.config.max_gas_per_tx,
            });
        }

        // Check account limit
        let sender_count = self.by_sender.get(&tx.sender).map(|m| m.len()).unwrap_or(0);
        if sender_count >= self.config.max_per_account {
            // Check for RBF opportunity when at account limit
            return self.try_rbf_at_limit(tx);
        }

        // Check pool capacity
        if self.by_hash.len() >= self.config.max_transactions {
            // Try to evict lowest priority transaction
            if !self.try_evict_for(&tx)? {
                return Err(MempoolError::PoolFull {
                    capacity: self.config.max_transactions,
                });
            }
        }

        // Check for RBF (same sender, same nonce)
        self.try_rbf_or_add(tx)
    }

    /// Attempts RBF when account is at its transaction limit.
    fn try_rbf_at_limit(&mut self, tx: MempoolTransaction) -> Result<(), MempoolError> {
        if !self.config.enable_rbf {
            return Err(MempoolError::AccountLimitReached {
                address: tx.sender,
                limit: self.config.max_per_account,
            });
        }

        let existing_hash = self
            .by_sender
            .get(&tx.sender)
            .and_then(|m| m.get(&tx.nonce))
            .copied();

        if let Some(hash) = existing_hash {
            let existing = self.by_hash.get(&hash).unwrap();
            if self.can_replace(existing, &tx)? {
                self.remove_internal(&hash)?;
                return self.add_internal(tx);
            }
        }

        Err(MempoolError::AccountLimitReached {
            address: tx.sender,
            limit: self.config.max_per_account,
        })
    }

    /// Attempts RBF if a transaction with same nonce exists, otherwise adds directly.
    fn try_rbf_or_add(&mut self, tx: MempoolTransaction) -> Result<(), MempoolError> {
        let existing_hash = self
            .by_sender
            .get(&tx.sender)
            .and_then(|m| m.get(&tx.nonce))
            .copied();

        let Some(hash) = existing_hash else {
            return self.add_internal(tx);
        };

        if !self.config.enable_rbf {
            return Err(MempoolError::RbfDisabled);
        }

        let existing = self.by_hash.get(&hash).unwrap();
        if !self.can_replace(existing, &tx)? {
            return Err(MempoolError::InsufficientFeeBump {
                old_price: self.by_hash.get(&hash).unwrap().gas_price,
                new_price: tx.gas_price,
                min_bump_percent: self.config.rbf_min_bump_percent,
            });
        }

        self.remove_internal(&hash)?;
        self.add_internal(tx)
    }

    /// Internal add without validation (assumes all checks passed).
    fn add_internal(&mut self, tx: MempoolTransaction) -> Result<(), MempoolError> {
        let hash = tx.hash;
        let sender = tx.sender;
        let nonce = tx.nonce;

        // Add to price index (only if pending)
        if tx.is_pending() {
            self.by_price
                .insert(PricedTransaction::new(tx.gas_price, tx.hash, tx.added_at));
        }

        // Add to sender index
        self.by_sender
            .entry(sender)
            .or_default()
            .insert(nonce, hash);

        // Add to main index
        self.by_hash.insert(hash, tx);

        Ok(())
    }

    /// Checks if new transaction can replace existing via RBF.
    fn can_replace(
        &self,
        existing: &MempoolTransaction,
        new: &MempoolTransaction,
    ) -> Result<bool, MempoolError> {
        // Cannot replace if existing is pending inclusion
        if existing.is_pending_inclusion() {
            return Err(MempoolError::TransactionPendingInclusion(existing.hash));
        }

        // Must be same sender and nonce
        if existing.sender != new.sender || existing.nonce != new.nonce {
            return Ok(false);
        }

        // Check minimum fee bump (using U256 arithmetic)
        let bump_multiplier = U256::from(100 + self.config.rbf_min_bump_percent);
        let min_new_price = existing.gas_price * bump_multiplier / U256::from(100);

        Ok(new.gas_price >= min_new_price)
    }

    /// Tries to evict the lowest priority transaction to make room.
    ///
    /// Only evicts if the new transaction has strictly higher priority (higher gas price
    /// or same gas price with earlier timestamp). Hash-based tie-breaking does NOT
    /// justify eviction to ensure deterministic behavior.
    fn try_evict_for(&mut self, new_tx: &MempoolTransaction) -> Result<bool, MempoolError> {
        // Get the lowest priority pending transaction
        let lowest = match self.by_price.iter().next_back() {
            Some(p) => p.clone(),
            None => return Ok(false), // No pending transactions to evict
        };

        // Get the lowest tx details for comparison
        let lowest_tx = match self.by_hash.get(&lowest.hash) {
            Some(tx) => tx,
            None => return Ok(false),
        };

        // New transaction must have STRICTLY higher priority to evict:
        // - Higher gas price, OR
        // - Same gas price but earlier timestamp
        // Hash tie-breaker does NOT justify eviction
        let has_higher_gas = new_tx.gas_price > lowest_tx.gas_price;
        let has_same_gas_earlier_time =
            new_tx.gas_price == lowest_tx.gas_price && new_tx.added_at < lowest_tx.added_at;

        if !has_higher_gas && !has_same_gas_earlier_time {
            return Ok(false); // New tx is not strictly higher priority
        }

        // Evict the lowest
        self.remove_internal(&lowest.hash)?;
        Ok(true)
    }

    /// Removes a transaction from the pool.
    pub fn remove(&mut self, hash: &Hash) -> Result<MempoolTransaction, MempoolError> {
        self.remove_internal(hash)
    }

    /// Internal remove implementation.
    fn remove_internal(&mut self, hash: &Hash) -> Result<MempoolTransaction, MempoolError> {
        let tx = self
            .by_hash
            .remove(hash)
            .ok_or(MempoolError::TransactionNotFound(*hash))?;

        // Remove from price index
        self.by_price
            .remove(&PricedTransaction::new(tx.gas_price, tx.hash, tx.added_at));

        // Remove from sender index
        if let Some(sender_txs) = self.by_sender.get_mut(&tx.sender) {
            sender_txs.remove(&tx.nonce);
            if sender_txs.is_empty() {
                self.by_sender.remove(&tx.sender);
            }
        }

        Ok(tx)
    }

    /// Gets the highest priority pending transactions for block building.
    ///
    /// INVARIANT-3: Only returns PENDING transactions, not PENDING_INCLUSION.
    ///
    /// Returns transactions in priority order (highest gas price first).
    /// Respects nonce ordering for each sender.
    pub fn get_for_block(&self, max_count: usize, max_gas: u64) -> Vec<&MempoolTransaction> {
        let mut result = Vec::new();
        let mut total_gas = 0u64;
        let mut sender_next_nonce: HashMap<Address, u64> = HashMap::new();

        // Iterate by price (highest first)
        for priced in self.by_price.iter() {
            if result.len() >= max_count {
                break;
            }

            let tx = match self.by_hash.get(&priced.hash) {
                Some(t) => t,
                None => continue,
            };

            // Check gas limit
            if total_gas.saturating_add(tx.gas_limit) > max_gas {
                continue;
            }

            // Check nonce ordering for this sender
            let expected_nonce = sender_next_nonce
                .get(&tx.sender)
                .copied()
                .unwrap_or_else(|| {
                    // Get the minimum nonce for this sender
                    self.by_sender
                        .get(&tx.sender)
                        .and_then(|m| m.keys().next().copied())
                        .unwrap_or(tx.nonce)
                });

            if tx.nonce != expected_nonce {
                // Skip this tx, it's not the next expected nonce
                continue;
            }

            result.push(tx);
            total_gas = total_gas.saturating_add(tx.gas_limit);
            sender_next_nonce.insert(tx.sender, tx.nonce + 1);
        }

        result
    }

    /// Proposes transactions for block inclusion (Phase 1 of Two-Phase Commit).
    ///
    /// Moves transactions from PENDING to PENDING_INCLUSION state.
    /// Transactions are NOT deleted.
    pub fn propose(&mut self, hashes: &[Hash], block_height: u64, now: Timestamp) -> ProposeResult {
        let mut result = ProposeResult::default();
        let mut proposed_hashes = Vec::new();

        for hash in hashes {
            let Some(tx) = self.by_hash.get_mut(hash) else {
                result.not_found.push(*hash);
                continue;
            };

            if tx.is_pending_inclusion() {
                result.already_pending.push(*hash);
                continue;
            }

            // Remove from price index (no longer available for proposals)
            self.by_price.remove(&PricedTransaction::new(
                tx.gas_price,
                tx.hash,
                tx.added_at,
            ));

            // Move to pending inclusion
            let _ = tx.propose(block_height, now);
            result.proposed_count += 1;
            proposed_hashes.push(*hash);
        }

        // Track the batch
        if !proposed_hashes.is_empty() {
            self.pending_batches.push(PendingInclusionBatch::new(
                block_height,
                now,
                proposed_hashes,
            ));
        }

        result
    }

    /// Confirms transaction inclusion (Phase 2a of Two-Phase Commit).
    ///
    /// Permanently deletes the confirmed transactions.
    pub fn confirm(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        use std::collections::HashSet;

        let mut confirmed = Vec::new();

        for hash in hashes {
            if let Ok(tx) = self.remove_internal(hash) {
                confirmed.push(tx.hash);
            }
        }

        // Clean up pending batches - O(1) lookup with HashSet
        let confirmed_set: HashSet<_> = confirmed.iter().collect();
        self.pending_batches.retain(|batch| {
            !batch
                .transaction_hashes
                .iter()
                .all(|h| confirmed_set.contains(h))
        });

        confirmed
    }

    /// Rolls back proposed transactions (Phase 2b of Two-Phase Commit).
    ///
    /// Returns transactions to PENDING state.
    pub fn rollback(&mut self, hashes: &[Hash]) -> Vec<Hash> {
        let mut rolled_back = Vec::new();

        for hash in hashes {
            let Some(tx) = self.by_hash.get_mut(hash) else {
                continue;
            };

            if !tx.is_pending_inclusion() {
                continue;
            }

            // Return to price index
            self.by_price.insert(PricedTransaction::new(
                tx.gas_price,
                tx.hash,
                tx.added_at,
            ));

            // Reset state
            let _ = tx.rollback();
            rolled_back.push(*hash);
        }

        // Remove from pending batches
        self.pending_batches
            .retain(|b| !b.transaction_hashes.iter().any(|h| hashes.contains(h)));

        rolled_back
    }
    /// Cleans up timed out pending inclusion transactions.
    ///
    /// INVARIANT-5: Transactions in PendingInclusion for > timeout are auto-rolled back.
    pub fn cleanup_timeouts(&mut self, now: Timestamp) -> Vec<Hash> {
        let timeout_ms = self.config.pending_inclusion_timeout_ms;

        // Find timed out transactions
        let timed_out: Vec<Hash> = self
            .by_hash
            .values()
            .filter(|tx| tx.is_timed_out(now, timeout_ms))
            .map(|tx| tx.hash)
            .collect();

        // Rollback all timed out transactions
        self.rollback(&timed_out)
    }

    /// Gets the number of transactions for a sender.
    pub fn sender_count(&self, sender: &Address) -> usize {
        self.by_sender.get(sender).map(|m| m.len()).unwrap_or(0)
    }

    /// Gets all transaction hashes for a sender.
    pub fn sender_transactions(&self, sender: &Address) -> Vec<Hash> {
        self.by_sender
            .get(sender)
            .map(|m| m.values().copied().collect())
            .unwrap_or_default()
    }

    /// Gets the mempool status.
    pub fn status(&self, now: Timestamp) -> MempoolStatus {
        let oldest_age = self
            .by_hash
            .values()
            .map(|tx| now.saturating_sub(tx.added_at))
            .max()
            .unwrap_or(0);

        let total_gas: u64 = self.by_hash.values().map(|tx| tx.gas_limit).sum();

        MempoolStatus {
            pending_count: self.pending_count(),
            pending_inclusion_count: self.pending_inclusion_count(),
            total_gas,
            memory_bytes: self.by_hash.len() * std::mem::size_of::<MempoolTransaction>(),
            oldest_tx_age_ms: oldest_age,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::SignedTransaction;

    fn create_signed_tx(sender_byte: u8, nonce: u64, gas_price: U256) -> SignedTransaction {
        SignedTransaction {
            from: [sender_byte; 20],
            to: Some([0xBB; 20]),
            value: U256::zero(),
            nonce,
            gas_price,
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        }
    }

    fn create_tx(sender_byte: u8, nonce: u64, gas_price: u64) -> MempoolTransaction {
        let signed_tx = create_signed_tx(sender_byte, nonce, U256::from(gas_price));
        MempoolTransaction::new(signed_tx, 1000)
    }

    fn create_tx_at(
        sender_byte: u8,
        nonce: u64,
        gas_price: u64,
        added_at: Timestamp,
    ) -> MempoolTransaction {
        let signed_tx = create_signed_tx(sender_byte, nonce, U256::from(gas_price));
        let mut tx = MempoolTransaction::new(signed_tx, added_at);
        tx.added_at = added_at;
        tx
    }

    // =========================================================================
    // TWO-PHASE COMMIT TESTS
    // =========================================================================

    #[test]
    fn test_propose_moves_to_pending_inclusion() {
        let mut pool = TransactionPool::with_defaults();
        let tx = create_tx(0xAA, 0, 2_000_000_000);
        let hash = tx.hash;

        pool.add(tx).unwrap();
        assert!(pool.get(&hash).unwrap().is_pending());

        let result = pool.propose(&[hash], 1, 2000);
        assert_eq!(result.proposed_count, 1);
        assert!(result.already_pending.is_empty());
        assert!(result.not_found.is_empty());

        // Transaction should now be pending inclusion
        assert!(pool.get(&hash).unwrap().is_pending_inclusion());

        // Transaction still exists in pool
        assert!(pool.contains(&hash));
    }

    #[test]
    fn test_confirm_deletes_transaction() {
        let mut pool = TransactionPool::with_defaults();
        let tx = create_tx(0xAA, 0, 2_000_000_000);
        let hash = tx.hash;

        pool.add(tx).unwrap();
        pool.propose(&[hash], 1, 2000);

        let confirmed = pool.confirm(&[hash]);
        assert_eq!(confirmed, vec![hash]);

        // Transaction should be deleted
        assert!(!pool.contains(&hash));
        assert!(pool.get(&hash).is_none());
    }

    #[test]
    fn test_rollback_returns_to_pending() {
        let mut pool = TransactionPool::with_defaults();
        let tx = create_tx(0xAA, 0, 2_000_000_000);
        let hash = tx.hash;

        pool.add(tx).unwrap();
        pool.propose(&[hash], 1, 2000);
        assert!(pool.get(&hash).unwrap().is_pending_inclusion());

        let rolled_back = pool.rollback(&[hash]);
        assert_eq!(rolled_back, vec![hash]);

        // Transaction should be back to pending
        assert!(pool.get(&hash).unwrap().is_pending());
    }

    #[test]
    fn test_pending_inclusion_excluded_from_proposal() {
        let mut pool = TransactionPool::with_defaults();
        let tx1 = create_tx(0xAA, 0, 2_000_000_000);
        let tx2 = create_tx(0xBB, 0, 1_000_000_000);
        let hash1 = tx1.hash;
        let hash2 = tx2.hash;

        pool.add(tx1).unwrap();
        pool.add(tx2).unwrap();

        // Propose tx1
        pool.propose(&[hash1], 1, 2000);

        // Get transactions for next block
        let available = pool.get_for_block(10, u64::MAX);

        // tx1 should NOT be in available (it's pending inclusion)
        assert!(!available.iter().any(|t| t.hash == hash1));
        // tx2 should be available
        assert!(available.iter().any(|t| t.hash == hash2));
    }

    #[test]
    fn test_pending_inclusion_timeout_triggers_rollback() {
        let config = MempoolConfig {
            pending_inclusion_timeout_ms: 1000,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);
        let tx = create_tx(0xAA, 0, 2_000_000_000);
        let hash = tx.hash;

        pool.add(tx).unwrap();
        pool.propose(&[hash], 1, 1000);

        // Not timed out yet
        let rolled_back = pool.cleanup_timeouts(1500);
        assert!(rolled_back.is_empty());
        assert!(pool.get(&hash).unwrap().is_pending_inclusion());

        // Now timed out
        let rolled_back = pool.cleanup_timeouts(2001);
        assert_eq!(rolled_back, vec![hash]);
        assert!(pool.get(&hash).unwrap().is_pending());
    }

    // =========================================================================
    // PRIORITY QUEUE TESTS
    // =========================================================================

    #[test]
    fn test_higher_gas_price_priority() {
        let mut pool = TransactionPool::with_defaults();

        let tx_low = create_tx(0xAA, 0, 1_000_000_000);
        let tx_high = create_tx(0xBB, 0, 2_000_000_000);
        let hash_high = tx_high.hash;

        pool.add(tx_low).unwrap();
        pool.add(tx_high).unwrap();

        let batch = pool.get_for_block(1, u64::MAX);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].hash, hash_high);
    }

    #[test]
    fn test_nonce_ordering_per_account() {
        let mut pool = TransactionPool::with_defaults();
        let sender = [0xAA; 20];

        // Add transactions with sequential nonces and decreasing gas prices
        // Higher nonce = lower gas price to test that nonces are respected
        let tx0 = create_tx(0xAA, 0, 3_000_000_000); // nonce 0, highest priority
        let tx1 = create_tx(0xAA, 1, 2_000_000_000); // nonce 1, medium priority
        let tx2 = create_tx(0xAA, 2, 1_000_000_000); // nonce 2, lowest priority

        // Add out of order
        pool.add(tx2).unwrap();
        pool.add(tx0).unwrap();
        pool.add(tx1).unwrap();

        let batch = pool.get_for_block(10, u64::MAX);

        // All three should be included in nonce order
        let sender_txs: Vec<_> = batch.iter().filter(|t| t.sender == sender).collect();
        assert_eq!(sender_txs.len(), 3);
        assert_eq!(sender_txs[0].nonce, 0);
        assert_eq!(sender_txs[1].nonce, 1);
        assert_eq!(sender_txs[2].nonce, 2);
    }

    #[test]
    fn test_high_nonce_skipped_if_gap_in_pool() {
        let mut pool = TransactionPool::with_defaults();

        // Add nonces 0 and 2 (gap at 1)
        let tx0 = create_tx(0xAA, 0, 1_000_000_000);
        let tx2 = create_tx(0xAA, 2, 2_000_000_000);

        pool.add(tx0).unwrap();
        pool.add(tx2).unwrap();

        let batch = pool.get_for_block(10, u64::MAX);

        // Only tx0 should be included - tx2 is skipped due to gap at nonce 1
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].nonce, 0);
    }

    #[test]
    fn test_nonce_chain_included_when_complete() {
        let mut pool = TransactionPool::with_defaults();

        // Add all nonces but out of order, with descending gas prices
        let tx0 = create_tx(0xAA, 0, 3_000_000_000);
        let tx1 = create_tx(0xAA, 1, 2_000_000_000);
        let tx2 = create_tx(0xAA, 2, 1_000_000_000);

        pool.add(tx1).unwrap();
        pool.add(tx2).unwrap();
        pool.add(tx0).unwrap();

        let batch = pool.get_for_block(10, u64::MAX);

        // All should be included now
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn test_fifo_for_same_gas_price() {
        let mut pool = TransactionPool::with_defaults();

        let tx1 = create_tx_at(0xAA, 0, 1_000_000_000, 1000);
        let tx2 = create_tx_at(0xBB, 0, 1_000_000_000, 2000);
        let hash1 = tx1.hash;

        pool.add(tx1).unwrap();
        pool.add(tx2).unwrap();

        let batch = pool.get_for_block(1, u64::MAX);
        assert_eq!(batch.len(), 1);
        // Earlier timestamp should come first
        assert_eq!(batch[0].hash, hash1);
    }

    // =========================================================================
    // REPLACE-BY-FEE TESTS
    // =========================================================================

    #[test]
    fn test_replace_by_fee_success() {
        let config = MempoolConfig {
            rbf_min_bump_percent: 10,
            enable_rbf: true,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let tx1 = create_tx(0xAA, 0, 1_000_000_000);
        let hash1 = tx1.hash;

        // tx2 has same sender/nonce but 15% higher gas price
        let tx2 = create_tx(0xAA, 0, 1_150_000_000);
        let hash2 = tx2.hash;

        pool.add(tx1).unwrap();
        assert!(pool.contains(&hash1));

        pool.add(tx2).unwrap();

        // tx1 should be replaced
        assert!(!pool.contains(&hash1));
        assert!(pool.contains(&hash2));
    }

    #[test]
    fn test_rbf_requires_minimum_bump() {
        let config = MempoolConfig {
            rbf_min_bump_percent: 10,
            enable_rbf: true,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let tx1 = create_tx(0xAA, 0, 1_000_000_000);
        let hash1 = tx1.hash;

        // tx2 has only 5% higher gas price (below 10% threshold)
        let tx2 = create_tx(0xAA, 0, 1_050_000_000);

        pool.add(tx1).unwrap();
        let result = pool.add(tx2);

        assert!(matches!(
            result,
            Err(MempoolError::InsufficientFeeBump { .. })
        ));
        assert!(pool.contains(&hash1));
    }

    #[test]
    fn test_rbf_disabled_config() {
        let config = MempoolConfig {
            enable_rbf: false,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let tx1 = create_tx(0xAA, 0, 1_000_000_000);
        let tx2 = create_tx(0xAA, 0, 2_000_000_000);

        pool.add(tx1).unwrap();
        let result = pool.add(tx2);

        assert!(matches!(result, Err(MempoolError::RbfDisabled)));
    }

    #[test]
    fn test_cannot_replace_pending_inclusion() {
        let mut pool = TransactionPool::with_defaults();

        let tx1 = create_tx(0xAA, 0, 1_000_000_000);
        let hash1 = tx1.hash;
        let tx2 = create_tx(0xAA, 0, 2_000_000_000);

        pool.add(tx1).unwrap();
        pool.propose(&[hash1], 1, 2000);

        let result = pool.add(tx2);
        assert!(matches!(
            result,
            Err(MempoolError::TransactionPendingInclusion(_))
        ));
    }

    // =========================================================================
    // EVICTION TESTS
    // =========================================================================

    #[test]
    fn test_evict_lowest_fee_when_full() {
        let config = MempoolConfig {
            max_transactions: 3,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let tx_low = create_tx(0xAA, 0, 1_000_000_000);
        let tx_med = create_tx(0xBB, 0, 1_500_000_000);
        let tx_high = create_tx(0xCC, 0, 2_000_000_000);
        let tx_higher = create_tx(0xDD, 0, 2_500_000_000);

        let hash_low = tx_low.hash;
        let hash_higher = tx_higher.hash;

        pool.add(tx_low).unwrap();
        pool.add(tx_med).unwrap();
        pool.add(tx_high).unwrap();

        assert_eq!(pool.len(), 3);

        // Adding higher priority should evict lowest
        pool.add(tx_higher).unwrap();

        assert_eq!(pool.len(), 3);
        assert!(!pool.contains(&hash_low));
        assert!(pool.contains(&hash_higher));
    }

    #[test]
    fn test_account_limit_enforcement() {
        let config = MempoolConfig {
            max_per_account: 3,
            enable_rbf: false,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        for i in 0..3 {
            let tx = create_tx(0xAA, i as u64, 1_000_000_000);
            pool.add(tx).unwrap();
        }

        // 4th transaction should fail
        let tx = create_tx(0xAA, 3, 1_000_000_000);
        let result = pool.add(tx);

        assert!(matches!(
            result,
            Err(MempoolError::AccountLimitReached { .. })
        ));
    }

    // =========================================================================
    // VALIDATION TESTS
    // =========================================================================

    #[test]
    fn test_reject_low_gas_price() {
        let config = MempoolConfig {
            min_gas_price: U256::from(1_000_000_000u64),
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let tx = create_tx(0xAA, 0, 500_000_000);
        let result = pool.add(tx);

        assert!(matches!(result, Err(MempoolError::GasPriceTooLow { .. })));
    }

    #[test]
    fn test_reject_high_gas_limit() {
        let config = MempoolConfig {
            max_gas_per_tx: 1_000_000,
            ..MempoolConfig::default()
        };
        let mut pool = TransactionPool::new(config);

        let mut signed_tx = create_signed_tx(0xAA, 0, U256::from(1_000_000_000u64));
        signed_tx.gas_limit = 2_000_000;
        let tx = MempoolTransaction::new(signed_tx, 1000);
        let result = pool.add(tx);

        assert!(matches!(result, Err(MempoolError::GasLimitTooHigh { .. })));
    }

    // =========================================================================
    // STATUS TESTS
    // =========================================================================

    #[test]
    fn test_status_counts() {
        let mut pool = TransactionPool::with_defaults();

        let tx1 = create_tx(0xAA, 0, 1_000_000_000);
        let tx2 = create_tx(0xBB, 0, 1_000_000_000);
        let hash1 = tx1.hash;

        pool.add(tx1).unwrap();
        pool.add(tx2).unwrap();

        let status = pool.status(3000);
        assert_eq!(status.pending_count, 2);
        assert_eq!(status.pending_inclusion_count, 0);

        pool.propose(&[hash1], 1, 2000);

        let status = pool.status(3000);
        assert_eq!(status.pending_count, 1);
        assert_eq!(status.pending_inclusion_count, 1);
    }

    #[test]
    fn test_address_is_20_bytes() {
        let tx = create_tx(0xAA, 0, 1_000_000_000);
        assert_eq!(tx.sender.len(), 20);
    }
}
