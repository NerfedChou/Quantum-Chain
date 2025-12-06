//! # Type-State Pattern for Two-Phase Commit
//!
//! This module implements compile-time state machine enforcement for transaction
//! state transitions. This prevents the "Wormhole Bypass" vulnerability where
//! direct field mutation could bypass the Two-Phase Commit coordinator.
//!
//! ## Security Background
//!
//! The Wormhole bridge hack ($320M) was caused by bypassing signature verification
//! via direct function calls. Similarly, our original MempoolTransaction design
//! had `pub state: TransactionState` which allowed bypassing the coordinator:
//!
//! ```ignore
//! // VULNERABLE: Direct field mutation bypasses coordinator
//! tx.state = TransactionState::PendingInclusion { ... };
//! ```
//!
//! ## Solution: Type-State Pattern
//!
//! With type-state, each state is a distinct type. State transitions consume
//! `self` and return a new type, making invalid transitions impossible:
//!
//! ```ignore
//! // SAFE: Type system enforces valid transitions
//! let pending: PendingTx = PendingTx::new(signed_tx);
//! let proposed: ProposedTx = pending.propose(block_height, now);  // Consumes pending
//! let _: PendingTx = proposed.rollback();  // Returns to pending state
//! // proposed.propose(...);  // COMPILE ERROR: proposed already consumed
//! ```
//!
//! ## Reference
//!
//! - SPEC-06 Section 1.3: Two-Phase Commit Protocol
//! - FINDINGS.md: Wormhole Bypass Pattern Analysis

use shared_types::{Address, Hash, SignedTransaction, U256};
use std::marker::PhantomData;

/// Timestamp in milliseconds since UNIX epoch.
pub type Timestamp = u64;

// =============================================================================
// STATE MARKERS (Zero-Sized Types)
// =============================================================================

/// Marker: Transaction is available for block inclusion.
#[derive(Debug, Clone, Copy)]
pub struct Pending;

/// Marker: Transaction has been proposed for a block, awaiting confirmation.
#[derive(Debug, Clone, Copy)]
pub struct Proposed;

/// Marker: Transaction is confirmed and ready for deletion.
#[derive(Debug, Clone, Copy)]
pub struct Confirmed;

// =============================================================================
// TYPE-STATE TRANSACTION
// =============================================================================

/// A transaction with compile-time enforced state.
///
/// The state is encoded in the type parameter `S`, which prevents invalid
/// state transitions at compile time.
///
/// ## State Machine
///
/// ```text
/// [Pending] ──propose──→ [Proposed] ──confirm──→ [Confirmed]
///                             │
///                             └── rollback ──→ [Pending]
/// ```
#[derive(Debug)]
pub struct TypeStateTx<S> {
    /// The signed transaction data
    pub transaction: SignedTransaction,
    /// Transaction hash (unique identifier)
    pub hash: Hash,
    /// Sender address
    pub sender: Address,
    /// Sender's nonce
    pub nonce: u64,
    /// Gas price
    pub gas_price: U256,
    /// Gas limit
    pub gas_limit: u64,
    /// Timestamp when added to pool
    pub added_at: Timestamp,
    /// State-specific data (private)
    state_data: StateData,
    /// Phantom data to hold the state type
    _state: PhantomData<S>,
}

/// State-specific data stored internally
#[derive(Debug, Clone)]
enum StateData {
    Pending,
    Proposed {
        block_height: u64,
        proposed_at: Timestamp,
    },
    Confirmed {
        block_height: u64,
        confirmed_at: Timestamp,
    },
}

// =============================================================================
// PENDING STATE IMPLEMENTATION
// =============================================================================

impl TypeStateTx<Pending> {
    /// Creates a new pending transaction.
    ///
    /// This is the ONLY entry point for creating transactions.
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
            added_at,
            state_data: StateData::Pending,
            _state: PhantomData,
        }
    }

    /// Propose this transaction for block inclusion.
    ///
    /// This CONSUMES the pending transaction and returns a proposed transaction.
    /// The original `PendingTx` cannot be used after this call.
    ///
    /// ## Security
    ///
    /// This enforces the state machine at compile time:
    /// - A pending tx can only be proposed once
    /// - A proposed tx cannot be re-proposed (different type)
    #[must_use = "The proposed transaction must be handled"]
    pub fn propose(self, block_height: u64, now: Timestamp) -> TypeStateTx<Proposed> {
        TypeStateTx {
            transaction: self.transaction,
            hash: self.hash,
            sender: self.sender,
            nonce: self.nonce,
            gas_price: self.gas_price,
            gas_limit: self.gas_limit,
            added_at: self.added_at,
            state_data: StateData::Proposed {
                block_height,
                proposed_at: now,
            },
            _state: PhantomData,
        }
    }

    /// Get the gas cost
    pub fn gas_cost(&self) -> U256 {
        self.gas_price * U256::from(self.gas_limit)
    }
}

// =============================================================================
// PROPOSED STATE IMPLEMENTATION
// =============================================================================

impl TypeStateTx<Proposed> {
    /// Confirm inclusion - transaction will be deleted.
    ///
    /// This CONSUMES the proposed transaction.
    #[must_use = "The confirmed transaction must be handled"]
    pub fn confirm(self, now: Timestamp) -> TypeStateTx<Confirmed> {
        let block_height = match self.state_data {
            StateData::Proposed { block_height, .. } => block_height,
            _ => unreachable!("Type system guarantees Proposed state"),
        };

        TypeStateTx {
            transaction: self.transaction,
            hash: self.hash,
            sender: self.sender,
            nonce: self.nonce,
            gas_price: self.gas_price,
            gas_limit: self.gas_limit,
            added_at: self.added_at,
            state_data: StateData::Confirmed {
                block_height,
                confirmed_at: now,
            },
            _state: PhantomData,
        }
    }

    /// Rollback to pending state (e.g., on timeout or block rejection).
    ///
    /// This CONSUMES the proposed transaction and returns a pending transaction.
    #[must_use = "The pending transaction must be handled"]
    pub fn rollback(self) -> TypeStateTx<Pending> {
        TypeStateTx {
            transaction: self.transaction,
            hash: self.hash,
            sender: self.sender,
            nonce: self.nonce,
            gas_price: self.gas_price,
            gas_limit: self.gas_limit,
            added_at: self.added_at,
            state_data: StateData::Pending,
            _state: PhantomData,
        }
    }

    /// Get the block height this transaction was proposed for.
    pub fn proposed_block(&self) -> u64 {
        match self.state_data {
            StateData::Proposed { block_height, .. } => block_height,
            _ => unreachable!("Type system guarantees Proposed state"),
        }
    }

    /// Get the timestamp when proposed.
    pub fn proposed_at(&self) -> Timestamp {
        match self.state_data {
            StateData::Proposed { proposed_at, .. } => proposed_at,
            _ => unreachable!("Type system guarantees Proposed state"),
        }
    }

    /// Check if this proposal has timed out.
    pub fn is_timed_out(&self, now: Timestamp, timeout_ms: u64) -> bool {
        now.saturating_sub(self.proposed_at()) >= timeout_ms
    }

    /// Get the gas cost
    pub fn gas_cost(&self) -> U256 {
        self.gas_price * U256::from(self.gas_limit)
    }
}

// =============================================================================
// CONFIRMED STATE IMPLEMENTATION
// =============================================================================

impl TypeStateTx<Confirmed> {
    /// Get the block height where confirmed.
    pub fn confirmed_block(&self) -> u64 {
        match self.state_data {
            StateData::Confirmed { block_height, .. } => block_height,
            _ => unreachable!("Type system guarantees Confirmed state"),
        }
    }

    /// Consume the confirmed transaction (for deletion).
    ///
    /// Returns the hash for cleanup purposes.
    pub fn consume(self) -> Hash {
        self.hash
    }
}

// =============================================================================
// TYPE-STATE POOL (Optional Advanced Usage)
// =============================================================================

/// A transaction pool that uses type-state for compile-time safety.
///
/// This is an alternative to the existing TransactionPool that provides
/// stronger compile-time guarantees about state transitions.
///
/// ## Usage
///
/// ```ignore
/// let mut pool = TypeStatePool::new();
///
/// // Add a pending transaction
/// let pending = TypeStateTx::new(signed_tx, now);
/// pool.add_pending(pending)?;
///
/// // Propose for block - returns ownership of proposed txs
/// let proposed: Vec<TypeStateTx<Proposed>> = pool.propose_batch(&hashes, block, now);
///
/// // Confirm or rollback
/// for tx in proposed {
///     if block_succeeded {
///         let confirmed = tx.confirm(now);
///         // confirmed.consume() - hash returned for cleanup
///     } else {
///         let pending = tx.rollback();
///         pool.add_pending(pending)?;  // Re-add to pool
///     }
/// }
/// ```
#[derive(Debug, Default)]
pub struct TypeStatePool {
    /// Pending transactions (available for block inclusion)
    pending: std::collections::HashMap<Hash, TypeStateTx<Pending>>,
    /// Count of transactions per sender
    sender_counts: std::collections::HashMap<Address, usize>,
}

impl TypeStatePool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a pending transaction to the pool.
    ///
    /// Takes ownership - the transaction cannot be used elsewhere.
    pub fn add_pending(&mut self, tx: TypeStateTx<Pending>) -> Result<(), &'static str> {
        if self.pending.contains_key(&tx.hash) {
            return Err("Duplicate transaction");
        }

        *self.sender_counts.entry(tx.sender).or_insert(0) += 1;
        self.pending.insert(tx.hash, tx);
        Ok(())
    }

    /// Remove and propose transactions for a block.
    ///
    /// Returns ownership of the proposed transactions.
    /// The caller MUST either confirm or rollback each transaction.
    #[must_use = "Proposed transactions must be confirmed or rolled back"]
    pub fn propose_batch(
        &mut self,
        hashes: &[Hash],
        block_height: u64,
        now: Timestamp,
    ) -> Vec<TypeStateTx<Proposed>> {
        hashes
            .iter()
            .filter_map(|hash| {
                self.pending.remove(hash).map(|tx| {
                    // Decrement sender count
                    if let Some(count) = self.sender_counts.get_mut(&tx.sender) {
                        *count = count.saturating_sub(1);
                    }
                    tx.propose(block_height, now)
                })
            })
            .collect()
    }

    /// Return a rolled-back transaction to the pool.
    pub fn return_pending(&mut self, tx: TypeStateTx<Pending>) -> Result<(), &'static str> {
        self.add_pending(tx)
    }

    /// Get number of pending transactions.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Check if a transaction exists.
    pub fn contains(&self, hash: &Hash) -> bool {
        self.pending.contains_key(hash)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_signed_tx(sender: u8, nonce: u64) -> SignedTransaction {
        SignedTransaction {
            from: [sender; 20],
            to: Some([0xBB; 20]),
            value: U256::zero(),
            nonce,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        }
    }

    #[test]
    fn test_pending_to_proposed() {
        let signed = create_signed_tx(0xAA, 0);
        let pending: TypeStateTx<Pending> = TypeStateTx::new(signed, 1000);

        assert_eq!(pending.added_at, 1000);

        let proposed: TypeStateTx<Proposed> = pending.propose(100, 2000);

        assert_eq!(proposed.proposed_block(), 100);
        assert_eq!(proposed.proposed_at(), 2000);
    }

    #[test]
    fn test_proposed_to_confirmed() {
        let signed = create_signed_tx(0xAA, 0);
        let pending = TypeStateTx::new(signed, 1000);
        let proposed = pending.propose(100, 2000);

        let confirmed: TypeStateTx<Confirmed> = proposed.confirm(3000);

        assert_eq!(confirmed.confirmed_block(), 100);
    }

    #[test]
    fn test_proposed_rollback() {
        let signed = create_signed_tx(0xAA, 0);
        let pending = TypeStateTx::new(signed, 1000);
        let hash = pending.hash;

        let proposed = pending.propose(100, 2000);
        let rolled_back: TypeStateTx<Pending> = proposed.rollback();

        // Hash should be preserved
        assert_eq!(rolled_back.hash, hash);
    }

    #[test]
    fn test_timeout_check() {
        let signed = create_signed_tx(0xAA, 0);
        let pending = TypeStateTx::new(signed, 1000);
        let proposed = pending.propose(100, 2000);

        // Not timed out
        assert!(!proposed.is_timed_out(2500, 1000));

        // Timed out
        assert!(proposed.is_timed_out(3001, 1000));
    }

    #[test]
    fn test_pool_basic_flow() {
        let mut pool = TypeStatePool::new();

        // Add pending
        let tx = TypeStateTx::new(create_signed_tx(0xAA, 0), 1000);
        let hash = tx.hash;
        pool.add_pending(tx).unwrap();

        assert_eq!(pool.pending_count(), 1);
        assert!(pool.contains(&hash));

        // Propose
        let proposed = pool.propose_batch(&[hash], 100, 2000);
        assert_eq!(proposed.len(), 1);
        assert_eq!(pool.pending_count(), 0);

        // Rollback
        let pending = proposed.into_iter().next().unwrap().rollback();
        pool.return_pending(pending).unwrap();

        assert_eq!(pool.pending_count(), 1);
    }

    #[test]
    fn test_pool_confirm_flow() {
        let mut pool = TypeStatePool::new();

        let tx = TypeStateTx::new(create_signed_tx(0xAA, 0), 1000);
        let hash = tx.hash;
        pool.add_pending(tx).unwrap();

        let proposed = pool.propose_batch(&[hash], 100, 2000);
        let confirmed = proposed.into_iter().next().unwrap().confirm(3000);

        // Consume returns the hash for cleanup
        let consumed_hash = confirmed.consume();
        assert_eq!(consumed_hash, hash);

        // Pool should be empty
        assert_eq!(pool.pending_count(), 0);
    }

    #[test]
    fn test_duplicate_rejected() {
        let mut pool = TypeStatePool::new();

        let tx1 = TypeStateTx::new(create_signed_tx(0xAA, 0), 1000);
        let tx2 = TypeStateTx::new(create_signed_tx(0xAA, 0), 1000); // Same tx

        pool.add_pending(tx1).unwrap();
        assert!(pool.add_pending(tx2).is_err());
    }

    // =========================================================================
    // COMPILE-TIME SAFETY TESTS
    // =========================================================================
    //
    // The following code blocks demonstrate compile-time errors that would
    // occur if someone tried to bypass the state machine. These are commented
    // out because they would fail to compile (which is the point!).
    //
    // ```compile_fail
    // // ERROR: Cannot propose a Proposed transaction
    // let proposed: TypeStateTx<Proposed> = ...;
    // proposed.propose(200, 3000);  // No `propose` method on TypeStateTx<Proposed>
    // ```
    //
    // ```compile_fail
    // // ERROR: Cannot rollback a Pending transaction
    // let pending: TypeStateTx<Pending> = ...;
    // pending.rollback();  // No `rollback` method on TypeStateTx<Pending>
    // ```
    //
    // ```compile_fail
    // // ERROR: Cannot confirm a Pending transaction
    // let pending: TypeStateTx<Pending> = ...;
    // pending.confirm(now);  // No `confirm` method on TypeStateTx<Pending>
    // ```
    //
    // ```compile_fail
    // // ERROR: Cannot use transaction after moving
    // let pending: TypeStateTx<Pending> = ...;
    // let proposed = pending.propose(100, 2000);  // Moves `pending`
    // pending.propose(200, 3000);  // ERROR: use of moved value
    // ```
}
