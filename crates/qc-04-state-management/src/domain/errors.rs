//! # State Management Error Types
//!
//! Defines all error conditions for state operations.
//! Each error is recoverable - no panics occur in production code.

use super::Address;
use thiserror::Error;

/// State management errors.
///
/// All errors are recoverable. None of these cause panics.
/// Callers should handle each variant appropriately.
#[derive(Debug, Error)]
pub enum StateError {
    /// Account does not exist in state trie.
    /// This is informational - new accounts have default state.
    #[error("Account not found: {address:?}")]
    AccountNotFound { address: Address },

    /// INVARIANT-1 violation: Balance cannot go negative.
    /// Transaction should be rejected, not applied.
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u128, available: u128 },

    /// INVARIANT-2 violation: Nonce doesn't match expected value.
    /// Transaction replay or out-of-order execution detected.
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },

    /// INVARIANT-2 violation: Nonce skips values.
    /// Transaction ordering error detected.
    #[error("Nonce gap: expected {expected}, got {actual}")]
    NonceGap { expected: u64, actual: u64 },

    /// DoS protection: Contract storage slot limit reached.
    /// Prevents unbounded state growth per contract.
    #[error("Storage limit exceeded for contract {address:?}")]
    StorageLimitExceeded { address: Address },

    /// Historical block state not available.
    /// Block may have been pruned or never existed.
    #[error("Block not found: height {height}")]
    BlockNotFound { height: u64 },

    /// IPC security: Sender not authorized for this operation.
    /// Per IPC-MATRIX.md authorization rules.
    #[error("Unauthorized sender: subsystem {0}")]
    UnauthorizedSender(u8),

    /// Persistence layer error.
    /// Wraps underlying storage errors (RocksDB, etc.).
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// INVARIANT-4 violation: Proof does not verify against root.
    /// Indicates data corruption or malicious proof.
    #[error("Proof verification failed")]
    ProofVerificationFailed,

    /// State root after applying block doesn't match expected.
    /// Critical consensus error - block should be rejected.
    #[error("State root mismatch: expected {expected:?}, got {actual:?}")]
    StateRootMismatch {
        expected: [u8; 32],
        actual: [u8; 32],
    },

    /// Requested snapshot height doesn't exist.
    /// May need to sync from peers or snapshot was pruned.
    #[error("Snapshot not found for block {height}")]
    SnapshotNotFound { height: u64 },

    /// DoS protection: Trie path too deep.
    /// Should never occur with 64-nibble keys.
    #[error("Trie depth exceeded: max {max}, attempted {attempted}")]
    TrieDepthExceeded { max: usize, attempted: usize },

    /// Failed to serialize/deserialize state data.
    /// Indicates data corruption or version mismatch.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Internal error: RwLock was poisoned.
    /// Indicates a previous thread panicked while holding lock.
    /// Should trigger node restart/recovery.
    #[error("State lock poisoned - internal consistency error")]
    LockPoisoned,

    /// Failed to generate Merkle proof for address.
    /// Indicates internal trie inconsistency.
    #[error("Proof generation failed for address {address:?}")]
    ProofGenerationFailed { address: Address },

    /// Requested rollback exceeds maximum depth.
    /// Older state has been pruned.
    #[error("Rollback depth exceeded: max {max_depth}, requested {requested}")]
    RollbackDepthExceeded { max_depth: u64, requested: u64 },
}
