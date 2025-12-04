//! Error types for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 6

use thiserror::Error;

/// Subsystem identifier
pub type SubsystemId = u8;

/// Finality subsystem errors
#[derive(Debug, Error)]
pub enum FinalityError {
    /// System is halted awaiting manual intervention
    #[error("System halted awaiting intervention - circuit breaker triggered")]
    SystemHalted,

    /// Invalid attestation signature
    #[error("Invalid attestation signature from validator {validator_id:?}")]
    InvalidSignature { validator_id: [u8; 32] },

    /// Unknown validator not in active set
    #[error("Unknown validator: {validator_id:?}")]
    UnknownValidator { validator_id: [u8; 32] },

    /// Attestation references unknown checkpoint
    #[error("Attestation for unknown checkpoint: epoch {epoch}")]
    UnknownCheckpoint { epoch: u64 },

    /// Conflicting attestation detected (slashable offense)
    #[error("Conflicting attestation detected - slashable offense")]
    ConflictingAttestation,

    /// Checkpoint not found
    #[error("Checkpoint not found: epoch {epoch}")]
    CheckpointNotFound { epoch: u64 },

    /// Unauthorized IPC sender
    #[error("Unauthorized sender: subsystem {sender_id}")]
    UnauthorizedSender { sender_id: SubsystemId },

    /// IPC security violation
    #[error("IPC security violation: {reason}")]
    IpcSecurityViolation { reason: String },

    /// Insufficient attestations for justification
    #[error("Insufficient attestations: have {have} stake, need {need}")]
    InsufficientAttestations { have: u128, need: u128 },

    /// Invalid checkpoint transition
    #[error("Invalid checkpoint transition: cannot go from {from:?} to {to:?}")]
    InvalidTransition { from: String, to: String },

    /// Stake query failed
    #[error("Stake query failed: {reason}")]
    StakeQueryFailed { reason: String },

    /// Storage error
    #[error("Storage error: {reason}")]
    StorageError { reason: String },

    /// Already finalized
    #[error("Block already finalized: {block_hash:?}")]
    AlreadyFinalized { block_hash: [u8; 32] },

    /// Epoch mismatch
    #[error("Epoch mismatch: expected {expected}, got {actual}")]
    EpochMismatch { expected: u64, actual: u64 },
}

/// Result type for finality operations
pub type FinalityResult<T> = Result<T, FinalityError>;
