//! Value objects for the Mempool subsystem.
//!
//! Immutable types used for ordering, indexing, and tracking transactions.

use super::entities::{Hash, Timestamp, U256};
use std::cmp::Ordering;

/// A transaction reference with price for ordering in the priority queue.
///
/// Implements `Ord` such that higher gas price = higher priority.
/// Ties are broken by timestamp (FIFO) then hash (deterministic).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PricedTransaction {
    /// Gas price (higher = higher priority).
    pub gas_price: U256,
    /// Transaction hash (unique identifier).
    pub hash: Hash,
    /// Timestamp when added (earlier = higher priority for ties).
    pub added_at: Timestamp,
}

impl PricedTransaction {
    /// Creates a new priced transaction reference.
    pub fn new(gas_price: U256, hash: Hash, added_at: Timestamp) -> Self {
        Self {
            gas_price,
            hash,
            added_at,
        }
    }
}

impl Ord for PricedTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher gas price = higher priority (so reverse comparison)
        other
            .gas_price
            .cmp(&self.gas_price)
            // Earlier timestamp = higher priority (FIFO for same price)
            .then_with(|| self.added_at.cmp(&other.added_at))
            // Deterministic tie-breaker using hash
            .then_with(|| self.hash.cmp(&other.hash))
    }
}

impl PartialOrd for PricedTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Tracking information for a pending inclusion batch.
///
/// Used to track which transactions were proposed for a specific block.
#[derive(Clone, Debug)]
pub struct PendingInclusionBatch {
    /// Target block height.
    pub block_height: u64,
    /// Block hash (optional, filled when storage confirms).
    pub block_hash: Option<Hash>,
    /// Timestamp when the batch was proposed.
    pub proposed_at: Timestamp,
    /// Transaction hashes in this batch.
    pub transaction_hashes: Vec<Hash>,
}

impl PendingInclusionBatch {
    /// Creates a new pending inclusion batch.
    pub fn new(block_height: u64, proposed_at: Timestamp, transaction_hashes: Vec<Hash>) -> Self {
        Self {
            block_height,
            block_hash: None,
            proposed_at,
            transaction_hashes,
        }
    }

    /// Checks if this batch has timed out.
    pub fn is_timed_out(&self, now: Timestamp, timeout_ms: u64) -> bool {
        now.saturating_sub(self.proposed_at) >= timeout_ms
    }
}

/// Short transaction ID for compact block relay.
///
/// Per System.md Subsystem 5: "short_txids: first 6 bytes XOR'd with salt"
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShortTxId(pub [u8; 6]);

impl ShortTxId {
    /// Computes a short TX ID from a full hash and a nonce/salt.
    ///
    /// Algorithm: XOR first 6 bytes of tx_hash with first 6 bytes of nonce hash.
    pub fn from_hash(tx_hash: &Hash, nonce: u64) -> Self {
        // Simple hash of nonce for XOR salt
        let nonce_bytes = nonce.to_le_bytes();
        let mut short_id = [0u8; 6];
        for i in 0..6 {
            short_id[i] = tx_hash[i] ^ nonce_bytes[i % 8];
        }
        Self(short_id)
    }

    /// Returns the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 6] {
        &self.0
    }
}

/// Mempool status snapshot.
#[derive(Clone, Debug, Default)]
pub struct MempoolStatus {
    /// Number of pending transactions.
    pub pending_count: usize,
    /// Number of transactions pending inclusion.
    pub pending_inclusion_count: usize,
    /// Total gas in the pool.
    pub total_gas: u64,
    /// Memory usage estimate in bytes.
    pub memory_bytes: usize,
    /// Age of oldest transaction in milliseconds.
    pub oldest_tx_age_ms: u64,
}

/// Result of proposing transactions for a block.
#[derive(Clone, Debug, Default)]
pub struct ProposeResult {
    /// Number of transactions successfully proposed.
    pub proposed_count: usize,
    /// Transactions that were already pending inclusion.
    pub already_pending: Vec<Hash>,
    /// Transactions that were not found in the pool.
    pub not_found: Vec<Hash>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priced_transaction_ordering_by_gas_price() {
        let low = PricedTransaction::new(U256::from(1_000_000_000u64), [1; 32], 1000);
        let high = PricedTransaction::new(U256::from(2_000_000_000u64), [2; 32], 1000);

        // Higher gas price should have higher priority (come first)
        assert!(high < low);
    }

    #[test]
    fn test_priced_transaction_ordering_fifo_for_same_price() {
        let earlier = PricedTransaction::new(U256::from(1_000_000_000u64), [1; 32], 1000);
        let later = PricedTransaction::new(U256::from(1_000_000_000u64), [2; 32], 2000);

        // Same gas price, earlier timestamp should have higher priority
        assert!(earlier < later);
    }

    #[test]
    fn test_priced_transaction_deterministic_tie_breaker() {
        let tx1 = PricedTransaction::new(U256::from(1_000_000_000u64), [1; 32], 1000);
        let tx2 = PricedTransaction::new(U256::from(1_000_000_000u64), [2; 32], 1000);

        // Same gas price and timestamp, use hash for deterministic ordering
        assert!(tx1 < tx2);
    }

    #[test]
    fn test_pending_inclusion_batch_timeout() {
        let batch = PendingInclusionBatch::new(1, 1000, vec![[1; 32], [2; 32]]);

        assert!(!batch.is_timed_out(1500, 1000));
        assert!(batch.is_timed_out(2000, 1000));
        assert!(batch.is_timed_out(3000, 1000));
    }

    #[test]
    fn test_pending_inclusion_batch_has_block_hash_field() {
        let mut batch = PendingInclusionBatch::new(1, 1000, vec![[1; 32]]);
        assert!(batch.block_hash.is_none());

        batch.block_hash = Some([0xAB; 32]);
        assert_eq!(batch.block_hash, Some([0xAB; 32]));
    }

    #[test]
    fn test_short_tx_id_computation() {
        let tx_hash = [0xAB; 32];
        let nonce = 12345u64;

        let short_id = ShortTxId::from_hash(&tx_hash, nonce);

        // Verify it's deterministic
        let short_id2 = ShortTxId::from_hash(&tx_hash, nonce);
        assert_eq!(short_id, short_id2);

        // Different nonce should produce different result
        let short_id3 = ShortTxId::from_hash(&tx_hash, 54321);
        assert_ne!(short_id, short_id3);
    }

    #[test]
    fn test_short_tx_id_different_hashes() {
        let hash1 = [0xAA; 32];
        let hash2 = [0xBB; 32];
        let nonce = 100u64;

        let id1 = ShortTxId::from_hash(&hash1, nonce);
        let id2 = ShortTxId::from_hash(&hash2, nonce);

        assert_ne!(id1, id2);
    }
}
