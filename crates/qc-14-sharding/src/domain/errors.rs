//! # Domain Errors
//!
//! Error types for Sharding subsystem.
//!
//! Reference: SPEC-14 Section 6 (Lines 531-554)

use thiserror::Error;

/// Shard identifier (u16 supports up to 65536 shards).
pub type ShardId = u16;

/// Hash type (32-byte keccak256).
pub type Hash = [u8; 32];

/// Address type (20-byte Ethereum-style).
pub type Address = [u8; 20];

/// Sharding error types.
#[derive(Debug, Error)]
pub enum ShardError {
    /// Unknown shard ID.
    #[error("Unknown shard: {0}")]
    UnknownShard(ShardId),

    /// Cross-shard lock acquisition failed.
    #[error("Cross-shard lock failed: {0}")]
    LockFailed(String),

    /// Cross-shard operation timed out.
    /// Reference: SPEC-14 Lines 650-652
    #[error("Cross-shard timeout after {0}s")]
    Timeout(u64),

    /// Invalid cross-shard proof.
    #[error("Invalid cross-shard proof")]
    InvalidProof,

    /// Insufficient validator signatures.
    #[error("Insufficient signatures: {got}/{required}")]
    InsufficientSignatures {
        /// Signatures received
        got: usize,
        /// Signatures required
        required: usize,
    },

    /// Transaction already processed.
    #[error("Transaction already processed: {0:?}")]
    AlreadyProcessed(Hash),

    /// Shard state inconsistency.
    #[error("Shard state inconsistency: {0}")]
    StateInconsistency(String),

    /// Invalid state transition.
    #[error("Invalid state transition: {from:?} -> {to:?}")]
    InvalidTransition {
        /// Current state
        from: String,
        /// Attempted state
        to: String,
    },

    /// Network error.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Epoch mismatch.
    #[error("Epoch mismatch: expected {expected}, got {got}")]
    EpochMismatch {
        /// Expected epoch
        expected: u64,
        /// Actual epoch
        got: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_shard_error() {
        let err = ShardError::UnknownShard(99);
        assert!(err.to_string().contains("99"));
    }

    #[test]
    fn test_lock_failed_error() {
        let err = ShardError::LockFailed("busy".to_string());
        assert!(err.to_string().contains("busy"));
    }

    #[test]
    fn test_timeout_error() {
        let err = ShardError::Timeout(30);
        assert!(err.to_string().contains("30"));
    }

    #[test]
    fn test_insufficient_signatures_error() {
        let err = ShardError::InsufficientSignatures {
            got: 5,
            required: 10,
        };
        assert!(err.to_string().contains("5/10"));
    }

    #[test]
    fn test_epoch_mismatch_error() {
        let err = ShardError::EpochMismatch {
            expected: 100,
            got: 99,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("99"));
    }
}
