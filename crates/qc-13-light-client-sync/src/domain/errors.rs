//! # Domain Errors
//!
//! Error types for Light Client Sync.
//!
//! Reference: SPEC-13 Section 6 (Lines 510-533)

use thiserror::Error;

/// Hash type alias (32-byte SHA-256)
pub type Hash = [u8; 32];

/// Light client error types.
#[derive(Debug, Error)]
pub enum LightClientError {
    /// Not enough full nodes available for multi-node consensus.
    /// Reference: System.md Line 644
    #[error("Not enough full nodes: {got} < {required}")]
    InsufficientNodes {
        /// Number of nodes found
        got: usize,
        /// Minimum required
        required: usize,
    },

    /// Multi-node consensus failed (nodes disagree).
    /// Reference: SPEC-13 Line 606
    #[error("Multi-node consensus failed: {0}")]
    ConsensusFailed(String),

    /// Merkle proof verification failed.
    /// Reference: System.md Line 645
    #[error("Merkle proof verification failed")]
    InvalidProof,

    /// Transaction not found in the chain.
    #[error("Transaction not found: {0:?}")]
    TransactionNotFound(Hash),

    /// Header chain mismatch with checkpoint.
    /// Reference: System.md Line 646
    #[error("Checkpoint mismatch at height {height}")]
    CheckpointMismatch {
        /// Block height where mismatch occurred
        height: u64,
    },

    /// Fork detected (nodes returned conflicting chains).
    /// Reference: SPEC-13 Line 161
    #[error("Header chain fork detected")]
    ForkDetected,

    /// Network error while communicating with full nodes.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Invalid header chain (broken parent link or height).
    #[error("Invalid header chain: {0}")]
    InvalidHeaderChain(String),

    /// Header not found.
    #[error("Header not found: {0:?}")]
    HeaderNotFound(Hash),

    /// Sync failed.
    #[error("Sync failed: {0}")]
    SyncFailed(String),

    /// Invalid block header.
    #[error("Invalid block header: {0}")]
    InvalidHeader(String),

    /// Insufficient confirmations.
    #[error("Insufficient confirmations: {got} < {required}")]
    InsufficientConfirmations {
        /// Confirmations received
        got: u64,
        /// Required confirmations
        required: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insufficient_nodes_error() {
        let err = LightClientError::InsufficientNodes {
            got: 2,
            required: 3,
        };
        assert!(err.to_string().contains("2 < 3"));
    }

    #[test]
    fn test_consensus_failed_error() {
        let err = LightClientError::ConsensusFailed("nodes disagree".to_string());
        assert!(err.to_string().contains("consensus"));
    }

    #[test]
    fn test_invalid_proof_error() {
        let err = LightClientError::InvalidProof;
        assert!(err.to_string().contains("Merkle"));
    }

    #[test]
    fn test_checkpoint_mismatch_error() {
        let err = LightClientError::CheckpointMismatch { height: 100000 };
        assert!(err.to_string().contains("100000"));
    }

    #[test]
    fn test_fork_detected_error() {
        let err = LightClientError::ForkDetected;
        assert!(err.to_string().contains("fork"));
    }
}
