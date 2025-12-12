//! Error types for Transaction Ordering
//!
//! Reference: SPEC-12 Section 6 (Lines 511-533)

use thiserror::Error;

/// All errors that can occur in transaction ordering
#[derive(Debug, Error)]
pub enum OrderingError {
    /// Cycle detected in dependency graph
    #[error("Cycle detected in dependency graph")]
    CycleDetected,

    /// Batch size exceeded limits
    #[error("Batch size exceeded: {size} > {max}")]
    BatchTooLarge { size: usize, max: usize },

    /// Edge count exceeded limits (anti-DoS)
    #[error("Edge count exceeded: {count} > {max}")]
    TooManyEdges { count: usize, max: usize },

    /// Access pattern analysis failed
    #[error("Access pattern analysis failed: {0}")]
    AnalysisFailed(String),

    /// Conflict detection failed
    #[error("Conflict detection failed: {0}")]
    ConflictDetectionFailed(String),

    /// Too many conflicts, falling back to sequential
    #[error("Too many conflicts: {percent}% exceeds threshold")]
    TooManyConflicts { percent: u8 },

    /// Unauthorized sender (IPC-MATRIX violation)
    #[error("Unauthorized sender: expected Consensus (8), got {sender_id}")]
    UnauthorizedSender { sender_id: u8 },

    /// Message timestamp out of bounds
    #[error("Message timestamp out of bounds")]
    StaleMessage,

    /// Replay attack detected
    #[error("Replay attack detected: duplicate nonce")]
    ReplayDetected,

    /// Empty transaction batch
    #[error("Empty transaction batch")]
    EmptyBatch,

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Analysis error for access pattern detection
#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error("Failed to decode transaction: {0}")]
    DecodeFailed(String),

    #[error("State read failed: {0}")]
    StateReadFailed(String),

    #[error("Timeout during analysis")]
    Timeout,
}

/// Conflict detection error
#[derive(Debug, Error)]
pub enum ConflictError {
    #[error("State query failed: {0}")]
    StateQueryFailed(String),

    #[error("Invalid transaction hash")]
    InvalidHash,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = OrderingError::BatchTooLarge {
            size: 2000,
            max: 1000,
        };
        assert_eq!(err.to_string(), "Batch size exceeded: 2000 > 1000");
    }

    #[test]
    fn test_cycle_detected_error() {
        let err = OrderingError::CycleDetected;
        assert_eq!(err.to_string(), "Cycle detected in dependency graph");
    }

    #[test]
    fn test_unauthorized_sender_error() {
        let err = OrderingError::UnauthorizedSender { sender_id: 5 };
        assert_eq!(
            err.to_string(),
            "Unauthorized sender: expected Consensus (8), got 5"
        );
    }
}
