//! Core domain entities for the Mempool subsystem.
//!
//! Defines the transaction state machine and related types as specified
//! in SPEC-06-MEMPOOL.md Section 2.1.

// Re-export from shared-types for convenience
pub use shared_types::{Address, Hash, SignedTransaction, U256};

/// Timestamp in milliseconds since UNIX epoch.
pub type Timestamp = u64;

/// Transaction state in the Two-Phase Commit protocol.
///
/// State machine (SPEC-06 Section 1.3):
/// ```text
/// [PENDING] ──propose──→ [PENDING_INCLUSION] ──confirm──→ [DELETED]
///                               │
///                               └── timeout/reject ──→ [PENDING] (rollback)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TransactionState {
    /// Transaction is available for block inclusion.
    #[default]
    Pending,
    /// Transaction has been proposed for a block, awaiting storage confirmation.
    PendingInclusion {
        /// Target block height for this proposal.
        block_height: u64,
        /// Timestamp when the transaction was proposed (ms).
        proposed_at: Timestamp,
    },
}

/// A transaction in the mempool with metadata.
///
/// Per SPEC-06 Section 2.1: Contains the full SignedTransaction.
///
/// INVARIANT-1: No two transactions can have the same hash.
/// INVARIANT-2: Transactions from same sender must be ordered by nonce.
#[derive(Clone, Debug)]
pub struct MempoolTransaction {
    /// The signed transaction (full transaction data).
    pub transaction: SignedTransaction,
    /// Transaction hash (unique identifier).
    pub hash: Hash,
    /// Sender address (20 bytes per IPC-MATRIX.md).
    pub sender: Address,
    /// Sender's nonce for this transaction.
    pub nonce: u64,
    /// Gas price for prioritization (U256 per SPEC-06).
    pub gas_price: U256,
    /// Gas limit for this transaction.
    pub gas_limit: u64,
    /// Current state in Two-Phase Commit.
    pub state: TransactionState,
    /// Timestamp when added to the pool (ms).
    pub added_at: Timestamp,
    /// Target block height (if in pending_inclusion state).
    pub target_block: Option<u64>,
}

impl MempoolTransaction {
    /// Creates a new pending transaction from a SignedTransaction.
    pub fn new(transaction: SignedTransaction, added_at: Timestamp) -> Self {
        let hash = transaction.hash();
        let sender = transaction.sender();
        let nonce = transaction.nonce;
        let gas_price = transaction.gas_price;
        let gas_limit = transaction.gas_limit;

        Self {
            transaction,
            hash,
            sender,
            nonce,
            gas_price,
            gas_limit,
            state: TransactionState::Pending,
            added_at,
            target_block: None,
        }
    }

    /// Returns the total gas cost (gas_price * gas_limit).
    pub fn gas_cost(&self) -> U256 {
        self.gas_price * U256::from(self.gas_limit)
    }

    /// Returns the total cost (value + gas_cost).
    pub fn total_cost(&self) -> U256 {
        self.transaction.value + self.gas_cost()
    }

    /// Returns true if the transaction is available for block inclusion.
    pub fn is_pending(&self) -> bool {
        matches!(self.state, TransactionState::Pending)
    }

    /// Returns true if the transaction is awaiting storage confirmation.
    pub fn is_pending_inclusion(&self) -> bool {
        matches!(self.state, TransactionState::PendingInclusion { .. })
    }

    /// Moves the transaction to PendingInclusion state.
    ///
    /// # Errors
    /// Returns error if transaction is already in PendingInclusion state.
    pub fn propose(&mut self, block_height: u64, now: Timestamp) -> Result<(), &'static str> {
        if self.is_pending_inclusion() {
            return Err("Transaction already pending inclusion");
        }
        self.state = TransactionState::PendingInclusion {
            block_height,
            proposed_at: now,
        };
        self.target_block = Some(block_height);
        Ok(())
    }

    /// Rolls back the transaction to Pending state.
    ///
    /// # Errors
    /// Returns error if transaction is not in PendingInclusion state.
    pub fn rollback(&mut self) -> Result<(), &'static str> {
        if !self.is_pending_inclusion() {
            return Err("Transaction not pending inclusion");
        }
        self.state = TransactionState::Pending;
        self.target_block = None;
        Ok(())
    }

    /// Checks if the pending inclusion has timed out.
    pub fn is_timed_out(&self, now: Timestamp, timeout_ms: u64) -> bool {
        match self.state {
            TransactionState::PendingInclusion { proposed_at, .. } => {
                now.saturating_sub(proposed_at) >= timeout_ms
            }
            _ => false,
        }
    }
}

/// Mempool configuration.
///
/// Default values per SPEC-06 Section 2.1.
#[derive(Clone, Debug)]
pub struct MempoolConfig {
    /// Maximum transactions in the pool.
    pub max_transactions: usize,
    /// Maximum transactions per account.
    pub max_per_account: usize,
    /// Minimum gas price (U256 per SPEC-06).
    pub min_gas_price: U256,
    /// Maximum gas per transaction.
    pub max_gas_per_tx: u64,
    /// Pending inclusion timeout (milliseconds).
    pub pending_inclusion_timeout_ms: u64,
    /// Nonce gap timeout (milliseconds).
    pub nonce_gap_timeout_ms: u64,
    /// Enable Replace-by-Fee.
    pub enable_rbf: bool,
    /// Minimum fee bump percentage for RBF.
    pub rbf_min_bump_percent: u64,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_transactions: 5000,
            max_per_account: 16,
            min_gas_price: U256::from(1_000_000_000u64), // 1 gwei
            max_gas_per_tx: 30_000_000,
            pending_inclusion_timeout_ms: 30_000, // 30 seconds
            nonce_gap_timeout_ms: 600_000,        // 10 minutes
            enable_rbf: true,
            rbf_min_bump_percent: 10,
        }
    }
}

impl MempoolConfig {
    /// Creates a minimal config for testing.
    pub fn for_testing() -> Self {
        Self {
            max_transactions: 100,
            max_per_account: 4,
            pending_inclusion_timeout_ms: 1000, // 1 second
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_signed_tx(sender_byte: u8, nonce: u64, gas_price: U256) -> SignedTransaction {
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

    fn create_test_tx(sender_byte: u8, nonce: u64, gas_price: U256) -> MempoolTransaction {
        let signed_tx = create_test_signed_tx(sender_byte, nonce, gas_price);
        MempoolTransaction::new(signed_tx, 1000)
    }

    #[test]
    fn test_transaction_state_default_is_pending() {
        let tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        assert!(tx.is_pending());
        assert!(!tx.is_pending_inclusion());
        assert!(tx.target_block.is_none());
    }

    #[test]
    fn test_propose_moves_to_pending_inclusion() {
        let mut tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        assert!(tx.is_pending());

        tx.propose(1, 2000).unwrap();

        assert!(!tx.is_pending());
        assert!(tx.is_pending_inclusion());
        assert_eq!(tx.target_block, Some(1));
        match tx.state {
            TransactionState::PendingInclusion {
                block_height,
                proposed_at,
            } => {
                assert_eq!(block_height, 1);
                assert_eq!(proposed_at, 2000);
            }
            _ => panic!("Expected PendingInclusion state"),
        }
    }

    #[test]
    fn test_propose_fails_if_already_pending_inclusion() {
        let mut tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        tx.propose(1, 2000).unwrap();

        let result = tx.propose(2, 3000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Transaction already pending inclusion");
    }

    #[test]
    fn test_rollback_returns_to_pending() {
        let mut tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        tx.propose(1, 2000).unwrap();
        assert!(tx.is_pending_inclusion());

        tx.rollback().unwrap();

        assert!(tx.is_pending());
        assert!(!tx.is_pending_inclusion());
        assert!(tx.target_block.is_none());
    }

    #[test]
    fn test_rollback_fails_if_not_pending_inclusion() {
        let mut tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        assert!(tx.is_pending());

        let result = tx.rollback();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Transaction not pending inclusion");
    }

    #[test]
    fn test_pending_inclusion_timeout_check() {
        let mut tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        tx.propose(1, 1000).unwrap();

        // Not timed out yet
        assert!(!tx.is_timed_out(1500, 1000));

        // Exactly at timeout
        assert!(tx.is_timed_out(2000, 1000));

        // Past timeout
        assert!(tx.is_timed_out(3000, 1000));
    }

    #[test]
    fn test_pending_transaction_never_times_out() {
        let tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        assert!(tx.is_pending());

        // Pending transactions should never timeout
        assert!(!tx.is_timed_out(1_000_000, 1000));
    }

    #[test]
    fn test_gas_cost_calculation() {
        let signed_tx = SignedTransaction {
            from: [0xAA; 20],
            to: Some([0xBB; 20]),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(2_000_000_000u64), // 2 gwei
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        };
        let tx = MempoolTransaction::new(signed_tx, 1000);

        assert_eq!(
            tx.gas_cost(),
            U256::from(2_000_000_000u64) * U256::from(21000u64)
        );
    }

    #[test]
    fn test_total_cost_calculation() {
        let signed_tx = SignedTransaction {
            from: [0xAA; 20],
            to: Some([0xBB; 20]),
            value: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            nonce: 0,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        };
        let tx = MempoolTransaction::new(signed_tx, 1000);

        let expected_gas_cost = U256::from(1_000_000_000u64) * U256::from(21000u64);
        let expected_total = U256::from(1_000_000_000_000_000_000u128) + expected_gas_cost;
        assert_eq!(tx.total_cost(), expected_total);
    }

    #[test]
    fn test_config_defaults() {
        let config = MempoolConfig::default();
        assert_eq!(config.max_transactions, 5000);
        assert_eq!(config.max_per_account, 16);
        assert_eq!(config.min_gas_price, U256::from(1_000_000_000u64));
        assert_eq!(config.pending_inclusion_timeout_ms, 30_000);
        assert!(config.enable_rbf);
        assert_eq!(config.rbf_min_bump_percent, 10);
    }

    #[test]
    fn test_address_is_20_bytes() {
        let tx = create_test_tx(0xAA, 0, U256::from(1_000_000_000u64));
        assert_eq!(tx.sender.len(), 20);
    }
}
