//! IPC Payloads for Transaction Ordering
//!
//! Reference: IPC-MATRIX.md Subsystem 12, Lines 1488-1518
//!
//! ## Security (Envelope-Only Identity - V2.2)
//!
//! CRITICAL: Payloads contain NO sender identity fields.
//! Identity is derived SOLELY from the AuthenticatedMessage envelope.

use serde::{Deserialize, Serialize};

// ============================================================
// INCOMING REQUESTS
// ============================================================

/// Request to order transactions for parallel execution.
///
/// ## IPC-MATRIX Reference: Lines 1513-1517
///
/// ## Security
///
/// MUST only accept from sender_id == SubsystemId::Consensus (8)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTransactionsRequest {
    /// Correlation ID for response tracking
    pub correlation_id: [u8; 16],
    /// Reply-to topic for response
    pub reply_to: String,
    /// Transactions to order (already validated by Consensus)
    pub transaction_hashes: Vec<[u8; 32]>,
    /// Sender addresses for nonce ordering
    pub senders: Vec<[u8; 20]>,
    /// Nonces for each transaction
    pub nonces: Vec<u64>,
    /// Read sets (address, key) for each transaction
    pub read_sets: Vec<Vec<([u8; 20], [u8; 32])>>,
    /// Write sets (address, key) for each transaction
    pub write_sets: Vec<Vec<([u8; 20], [u8; 32])>>,
}

// ============================================================
// OUTGOING RESPONSES
// ============================================================

/// Response with ordered transactions.
///
/// ## IPC-MATRIX Reference: Lines 1489-1508
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderTransactionsResponse {
    /// Correlation ID from request
    pub correlation_id: [u8; 16],
    /// Whether ordering succeeded
    pub success: bool,
    /// Ordered transaction hashes (by parallel group)
    pub parallel_groups: Vec<Vec<[u8; 32]>>,
    /// Metrics
    pub metrics: OrderingMetrics,
    /// Error message (if failed)
    pub error: Option<String>,
}

/// Ordering metrics for observability.
///
/// ## IPC-MATRIX Reference: Lines 1502-1508
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderingMetrics {
    /// Total transactions ordered
    pub total_transactions: u32,
    /// Number of parallel groups created
    pub parallel_groups: u32,
    /// Maximum parallelism achieved
    pub max_parallelism: u32,
    /// Conflicts detected between transactions
    pub conflicts_detected: u32,
    /// Time taken for ordering (ms)
    pub ordering_time_ms: u64,
}

// ============================================================
// OUTGOING REQUESTS (to State Management)
// ============================================================

/// Request to detect conflicts between transactions.
///
/// Sent to Subsystem 4 (State Management) per IPC-MATRIX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetectionRequest {
    /// Correlation ID for response tracking
    pub correlation_id: [u8; 16],
    /// Reply-to topic for response
    pub reply_to: String,
    /// Transaction access patterns to check
    pub access_patterns: Vec<TransactionAccessPattern>,
}

/// Access pattern for a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionAccessPattern {
    /// Transaction hash
    pub tx_hash: [u8; 32],
    /// Storage locations read
    pub reads: Vec<([u8; 20], [u8; 32])>,
    /// Storage locations written
    pub writes: Vec<([u8; 20], [u8; 32])>,
}

/// Response from State Management with detected conflicts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetectionResponse {
    /// Correlation ID from request
    pub correlation_id: [u8; 16],
    /// Detected conflicts (tx_index1, tx_index2, conflict_type)
    pub conflicts: Vec<(usize, usize, u8)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_transactions_request_serialization() {
        let request = OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "response_topic".to_string(),
            transaction_hashes: vec![[2u8; 32]],
            senders: vec![[3u8; 20]],
            nonces: vec![0],
            read_sets: vec![vec![]],
            write_sets: vec![vec![]],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("correlation_id"));
    }

    #[test]
    fn test_ordering_metrics_default() {
        let metrics = OrderingMetrics::default();
        assert_eq!(metrics.total_transactions, 0);
        assert_eq!(metrics.parallel_groups, 0);
    }
}
