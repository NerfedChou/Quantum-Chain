use super::Address;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("Account not found: {address:?}")]
    AccountNotFound { address: Address },

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: u128, available: u128 },

    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },

    #[error("Nonce gap: expected {expected}, got {actual}")]
    NonceGap { expected: u64, actual: u64 },

    #[error("Storage limit exceeded for contract {address:?}")]
    StorageLimitExceeded { address: Address },

    #[error("Block not found: height {height}")]
    BlockNotFound { height: u64 },

    #[error("Unauthorized sender: subsystem {0}")]
    UnauthorizedSender(u8),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Proof verification failed")]
    ProofVerificationFailed,

    #[error("State root mismatch: expected {expected:?}, got {actual:?}")]
    StateRootMismatch { expected: [u8; 32], actual: [u8; 32] },

    #[error("Snapshot not found for block {height}")]
    SnapshotNotFound { height: u64 },

    #[error("Trie depth exceeded: max {max}, attempted {attempted}")]
    TrieDepthExceeded { max: usize, attempted: usize },

    #[error("Serialization error: {0}")]
    SerializationError(String),
}
