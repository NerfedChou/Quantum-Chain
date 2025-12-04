//! # IPC Message Payloads
//!
//! Request/response types for inter-subsystem communication.
//! All payloads are wrapped in `AuthenticatedMessage<T>` per Architecture.md v2.3.

use crate::domain::{Hash, MempoolStatus};
use serde::{Deserialize, Serialize};
use shared_types::SignedTransaction;
use uuid::Uuid;

/// Request to add a pre-verified transaction.
///
/// # Security
/// - Sender: Subsystem 10 (Signature Verification) ONLY
/// - The transaction MUST have been signature-verified before sending
///
/// Per SPEC-06: Contains the full SignedTransaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTransactionRequest {
    /// Correlation ID for request tracking.
    pub correlation_id: Uuid,
    /// The full signed transaction (per SPEC-06).
    pub transaction: SignedTransaction,
    /// Whether the signature was verified as valid.
    pub signature_verified: bool,
}

/// Response to add transaction request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTransactionResponse {
    /// Correlation ID matching the request.
    pub correlation_id: Uuid,
    /// Whether the transaction was accepted.
    pub accepted: bool,
    /// The transaction hash if accepted.
    pub tx_hash: Option<Hash>,
    /// Error message if rejected.
    pub error: Option<String>,
}

/// Request to get transactions for block building.
///
/// # Security
/// - Sender: Subsystem 8 (Consensus) ONLY
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionsRequest {
    /// Correlation ID for request tracking.
    pub correlation_id: Uuid,
    /// Maximum number of transactions to return.
    pub max_count: u32,
    /// Maximum total gas for returned transactions.
    pub max_gas: u64,
    /// Target block height.
    pub target_block_height: u64,
}

/// Response containing transactions for block building.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionsResponse {
    /// Correlation ID matching the request.
    pub correlation_id: Uuid,
    /// Transaction hashes in priority order.
    pub tx_hashes: Vec<Hash>,
    /// Total gas of returned transactions.
    pub total_gas: u64,
}

/// Confirmation that transactions were stored in a block.
///
/// # Security
/// - Sender: Subsystem 2 (Block Storage) ONLY
/// - This is Phase 2a of the Two-Phase Commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStorageConfirmation {
    /// Correlation ID for tracking.
    pub correlation_id: Uuid,
    /// The stored block hash.
    pub block_hash: Hash,
    /// The stored block height.
    pub block_height: u64,
    /// Transaction hashes that were included.
    pub included_transactions: Vec<Hash>,
    /// Timestamp when stored.
    pub storage_timestamp: u64,
}

/// Notification that a block was rejected.
///
/// # Security
/// - Sender: Subsystems 2 (Block Storage) or 8 (Consensus) ONLY
/// - This is Phase 2b of the Two-Phase Commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRejectedNotification {
    /// Correlation ID for tracking.
    pub correlation_id: Uuid,
    /// The rejected block hash.
    pub block_hash: Hash,
    /// The rejected block height.
    pub block_height: u64,
    /// Transaction hashes that should be rolled back.
    pub affected_transactions: Vec<Hash>,
    /// Reason for rejection.
    pub rejection_reason: BlockRejectionReason,
}

/// Reasons for block rejection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockRejectionReason {
    /// Consensus voted to reject.
    ConsensusRejected,
    /// Block storage failed.
    StorageFailure,
    /// Proposal timed out.
    Timeout,
    /// Chain reorganization occurred.
    Reorg,
}

/// Request to remove transactions from the pool.
///
/// # Security
/// - Sender: Subsystem 8 (Consensus) ONLY
/// - Only for Invalid or Expired reasons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveTransactionsRequest {
    /// Correlation ID for request tracking.
    pub correlation_id: Uuid,
    /// Transaction hashes to remove.
    pub tx_hashes: Vec<Hash>,
    /// Reason for removal.
    pub reason: RemovalReason,
}

/// Reasons for transaction removal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RemovalReason {
    /// Transaction is invalid (e.g., insufficient balance after state change).
    Invalid,
    /// Transaction has expired (too old).
    Expired,
}

/// Response to remove transactions request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveTransactionsResponse {
    /// Correlation ID matching the request.
    pub correlation_id: Uuid,
    /// Number of transactions removed.
    pub removed_count: usize,
    /// Hashes of removed transactions.
    pub removed: Vec<Hash>,
}

/// Request for mempool status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStatusRequest {
    /// Correlation ID for request tracking.
    pub correlation_id: Uuid,
}

/// Response containing mempool status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStatusResponse {
    /// Correlation ID matching the request.
    pub correlation_id: Uuid,
    /// Current mempool status.
    pub status: MempoolStatusPayload,
}

/// Mempool status for IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolStatusPayload {
    /// Number of pending transactions.
    pub pending_count: u32,
    /// Number of transactions pending inclusion.
    pub pending_inclusion_count: u32,
    /// Total gas in the pool.
    pub total_gas: u64,
    /// Memory usage in bytes.
    pub memory_bytes: u64,
}

impl From<MempoolStatus> for MempoolStatusPayload {
    fn from(s: MempoolStatus) -> Self {
        Self {
            pending_count: s.pending_count as u32,
            pending_inclusion_count: s.pending_inclusion_count as u32,
            total_gas: s.total_gas,
            memory_bytes: s.memory_bytes as u64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::U256;

    fn create_test_signed_tx() -> SignedTransaction {
        SignedTransaction {
            from: [0xBB; 20],
            to: Some([0xCC; 20]),
            value: U256::from(1_000_000u64),
            nonce: 5,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            data: vec![1, 2, 3],
            signature: [0u8; 64],
        }
    }

    #[test]
    fn test_add_transaction_request_serialization() {
        let req = AddTransactionRequest {
            correlation_id: Uuid::new_v4(),
            transaction: create_test_signed_tx(),
            signature_verified: true,
        };

        let json = serde_json::to_string(&req).unwrap();
        let decoded: AddTransactionRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.transaction.nonce, 5);
        assert!(decoded.signature_verified);
    }

    #[test]
    fn test_block_rejection_reason() {
        let reason = BlockRejectionReason::ConsensusRejected;
        let json = serde_json::to_string(&reason).unwrap();
        assert!(json.contains("ConsensusRejected"));
    }
}
