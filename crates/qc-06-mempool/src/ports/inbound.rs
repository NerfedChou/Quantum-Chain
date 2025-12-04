//! # Inbound Port - MempoolApi
//!
//! Primary driving port exposing the transaction pool API.
//!
//! ## Authorization (IPC-MATRIX.md)
//!
//! | Method | Authorized Caller |
//! |--------|-------------------|
//! | `add_transaction` | Subsystem 10 (Signature Verification) |
//! | `get_transactions_for_block` | Subsystem 8 (Consensus) |
//! | `confirm_inclusion` | Subsystem 2 (Block Storage) |
//! | `rollback_proposal` | Subsystems 2, 8 |

use crate::domain::{
    Hash, MempoolError, MempoolStatus, MempoolTransaction, ProposeResult, ShortTxId,
    TransactionState,
};

/// Primary API for the Mempool subsystem.
///
/// This is the driving port that allows other subsystems to interact
/// with the transaction pool.
///
/// # Security (IPC-MATRIX.md)
///
/// - `add_transaction`: Only Subsystem 10 (Signature Verification)
/// - `get_transactions_for_block`: Only Subsystem 8 (Consensus)
/// - `confirm_inclusion`: Only Subsystem 2 (Block Storage)
/// - `rollback_proposal`: Only Subsystems 2, 8
///
/// # Example
///
/// ```rust,ignore
/// use qc_06_mempool::ports::MempoolApi;
///
/// async fn example(mempool: &impl MempoolApi) {
///     // Get transactions for a new block
///     let txs = mempool.get_transactions_for_block(100, 30_000_000);
///     
///     // Propose them
///     let hashes: Vec<_> = txs.iter().map(|t| t.hash).collect();
///     let result = mempool.propose_transactions(&hashes, 1);
///     
///     // After block is stored, confirm inclusion
///     mempool.confirm_inclusion(1, [0xAB; 32], &hashes);
/// }
/// ```
pub trait MempoolApi: Send + Sync {
    /// Adds a pre-verified transaction to the pool.
    ///
    /// # Security
    /// Only Subsystem 10 (Signature Verification) is authorized to call this.
    /// The transaction MUST have been signature-verified before calling.
    ///
    /// # Errors
    /// - `DuplicateTransaction`: Transaction hash already exists
    /// - `GasPriceTooLow`: Below minimum gas price
    /// - `AccountLimitReached`: Sender has too many pending transactions
    /// - `PoolFull`: Pool at capacity and new tx doesn't qualify for eviction
    fn add_transaction(&mut self, tx: MempoolTransaction) -> Result<Hash, MempoolError>;

    /// Gets the highest priority pending transactions for block building.
    ///
    /// # Security
    /// Only Subsystem 8 (Consensus) is authorized to call this.
    ///
    /// Returns transactions in priority order (highest gas price first).
    /// Respects nonce ordering for each sender.
    /// Only returns PENDING transactions, not PENDING_INCLUSION.
    fn get_transactions_for_block(&self, max_count: usize, max_gas: u64)
        -> Vec<MempoolTransaction>;

    /// Proposes transactions for block inclusion (Phase 1 of Two-Phase Commit).
    ///
    /// Moves transactions from PENDING to PENDING_INCLUSION state.
    /// Transactions are NOT deleted - they remain in the pool until confirmed.
    ///
    /// # Security
    /// Only Subsystem 8 (Consensus) is authorized to call this.
    fn propose_transactions(&mut self, tx_hashes: &[Hash], block_height: u64) -> ProposeResult;

    /// Confirms that transactions were included in a stored block (Phase 2a).
    ///
    /// This permanently deletes the transactions from the pool.
    ///
    /// # Security
    /// Only Subsystem 2 (Block Storage) is authorized to call this.
    fn confirm_inclusion(
        &mut self,
        block_height: u64,
        block_hash: Hash,
        tx_hashes: &[Hash],
    ) -> Vec<Hash>;

    /// Rolls back proposed transactions (Phase 2b).
    ///
    /// Returns transactions from PENDING_INCLUSION back to PENDING state.
    /// Used when a block is rejected or times out.
    ///
    /// # Security
    /// Only Subsystems 2 (Block Storage) and 8 (Consensus) are authorized.
    fn rollback_proposal(&mut self, tx_hashes: &[Hash]) -> Vec<Hash>;

    /// Gets a transaction by hash.
    fn get_transaction(&self, hash: &Hash) -> Option<MempoolTransaction>;

    /// Gets the state of a transaction.
    fn get_transaction_state(&self, hash: &Hash) -> Option<TransactionState>;

    /// Checks if a transaction exists in the pool.
    fn contains(&self, hash: &Hash) -> bool;

    /// Removes invalid or expired transactions.
    ///
    /// # Security
    /// Only Subsystem 8 (Consensus) is authorized to remove transactions
    /// for Invalid/Expired reasons.
    fn remove_transactions(&mut self, hashes: &[Hash]) -> Vec<Hash>;

    /// Gets the current mempool status.
    fn get_status(&self) -> MempoolStatus;

    /// Gets transactions for compact block reconstruction.
    ///
    /// Returns transactions matching the given hashes, in the same order.
    /// Missing transactions are represented as None.
    ///
    /// # Security
    /// Used by Subsystem 5 (Block Propagation) for compact block relay.
    fn get_transactions_by_hashes(&self, hashes: &[Hash]) -> Vec<Option<MempoolTransaction>>;

    /// Calculates short transaction IDs for compact block relay.
    ///
    /// Per System.md Subsystem 5: "short_txids: first 6 bytes XOR'd with salt"
    fn calculate_short_ids(&self, tx_hashes: &[Hash], nonce: u64) -> Vec<ShortTxId>;

    /// Cleans up timed out pending inclusion transactions.
    ///
    /// Should be called periodically (e.g., every second).
    fn cleanup_timeouts(&mut self) -> Vec<Hash>;

    /// Gets the number of transactions in the pool.
    fn len(&self) -> usize;

    /// Returns true if the pool is empty.
    fn is_empty(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test that the trait is object-safe (can be used as dyn MempoolApi)
    fn _assert_object_safe(_: &dyn MempoolApi) {}
}
