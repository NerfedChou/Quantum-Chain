//! Error types for Consensus subsystem
//!
//! Reference: SPEC-08-CONSENSUS.md Section 6

use super::ValidatorId;
use shared_types::Hash;

/// Consensus error types
///
/// Reference: SPEC-08 Section 6
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Unknown parent block: {0:?}")]
    UnknownParent(Hash),

    #[error("Invalid block signature")]
    InvalidSignature,

    #[error("Insufficient attestations: {got}%, required {required}%")]
    InsufficientAttestations { got: u8, required: u8 },

    #[error("Invalid block height: expected {expected}, got {actual}")]
    InvalidHeight { expected: u64, actual: u64 },

    #[error("Invalid timestamp: block {block} <= parent {parent}")]
    InvalidTimestamp { block: u64, parent: u64 },

    #[error("Block gas exceeds limit: {used} > {limit}")]
    GasLimitExceeded { used: u64, limit: u64 },

    #[error("Too many transactions: {count} > {limit}")]
    TooManyTransactions { count: usize, limit: usize },

    #[error("Unauthorized sender: expected {expected}, got {actual}")]
    UnauthorizedSender { expected: u8, actual: u8 },

    #[error("PBFT view mismatch: expected {expected}, got {actual}")]
    ViewMismatch { expected: u64, actual: u64 },

    #[error("Unknown validator: {0:?}")]
    UnknownValidator(ValidatorId),

    #[error("Signature verification failed for validator: {0:?}")]
    SignatureVerificationFailed(ValidatorId),

    #[error("Duplicate vote from validator: {0:?}")]
    DuplicateVote(ValidatorId),

    #[error("Stale block from epoch {block_epoch}, current epoch is {current_epoch}")]
    StaleBlock {
        block_epoch: u64,
        current_epoch: u64,
    },

    #[error("Timestamp too far in future: {timestamp}, current is {current}")]
    FutureTimestamp { timestamp: u64, current: u64 },

    #[error("Event bus error: {0}")]
    EventBusError(String),

    #[error("Mempool error: {0}")]
    MempoolError(String),

    #[error("State error: {0}")]
    StateError(String),

    #[error("IPC security error: {0}")]
    IpcSecurityError(String),

    #[error("Block already validated: {0:?}")]
    AlreadyValidated(Hash),

    #[error("Genesis block cannot have parent")]
    GenesisWithParent,

    #[error("Missing genesis block")]
    MissingGenesis,

    #[error("Invalid proposer: {0:?} not in validator set")]
    InvalidProposer(ValidatorId),

    #[error("Proposer did not attest: {0:?}")]
    ProposerDidNotAttest(ValidatorId),

    #[error("Invalid signature format for validator: {0:?}")]
    InvalidSignatureFormat(ValidatorId),

    #[error("Extra data too large: {size} bytes > {limit} bytes")]
    ExtraDataTooLarge { size: usize, limit: usize },
}

/// Result type for consensus operations
pub type ConsensusResult<T> = Result<T, ConsensusError>;
