//! # Domain Errors
//!
//! Error types for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 3.1: IndexingError enum
//! - Section 4.3: IndexingErrorPayload for IPC

use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// Errors that can occur during indexing operations.
///
/// ## SPEC-03 Section 3.1
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexingError {
    /// Transaction hash not found in index.
    TransactionNotFound { tx_hash: Hash },
    /// Merkle tree was evicted from cache, must rebuild.
    TreeNotCached { block_hash: Hash },
    /// Transaction index out of bounds.
    InvalidIndex { index: usize, max: usize },
    /// Empty block (no transactions to index).
    EmptyBlock { block_hash: Hash },
    /// Serialization error.
    SerializationError { message: String },
    /// Storage error.
    StorageError { message: String },
    /// Unauthorized sender for this operation.
    UnauthorizedSender { sender_id: u8, expected: u8 },
    /// Communication error with another subsystem.
    CommunicationError { message: String },
    /// Request timed out.
    Timeout { operation: String },
}

impl std::fmt::Display for IndexingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TransactionNotFound { tx_hash } => {
                write!(f, "Transaction not found: {:?}", &tx_hash[..8])
            }
            Self::TreeNotCached { block_hash } => {
                write!(
                    f,
                    "Merkle tree not cached for block: {:?}",
                    &block_hash[..8]
                )
            }
            Self::InvalidIndex { index, max } => {
                write!(f, "Invalid index {} (max: {})", index, max)
            }
            Self::EmptyBlock { block_hash } => {
                write!(f, "Empty block: {:?}", &block_hash[..8])
            }
            Self::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            Self::StorageError { message } => {
                write!(f, "Storage error: {}", message)
            }
            Self::UnauthorizedSender {
                sender_id,
                expected,
            } => {
                write!(
                    f,
                    "Unauthorized sender {} (expected: {})",
                    sender_id, expected
                )
            }
            Self::CommunicationError { message } => {
                write!(f, "Communication error: {}", message)
            }
            Self::Timeout { operation } => {
                write!(f, "Operation timed out: {}", operation)
            }
        }
    }
}

impl std::error::Error for IndexingError {}

/// Serializable indexing error for IPC.
///
/// ## SPEC-03 Section 4.3
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexingErrorPayload {
    pub error_type: IndexingErrorType,
    pub message: String,
    pub transaction_hash: Option<Hash>,
    pub block_hash: Option<Hash>,
}

impl From<IndexingError> for IndexingErrorPayload {
    fn from(err: IndexingError) -> Self {
        match err {
            IndexingError::TransactionNotFound { tx_hash } => Self {
                error_type: IndexingErrorType::TransactionNotFound,
                message: "Transaction not found".to_string(),
                transaction_hash: Some(tx_hash),
                block_hash: None,
            },
            IndexingError::TreeNotCached { block_hash } => Self {
                error_type: IndexingErrorType::TreeNotCached,
                message: "Merkle tree not cached".to_string(),
                transaction_hash: None,
                block_hash: Some(block_hash),
            },
            IndexingError::InvalidIndex { index, max } => Self {
                error_type: IndexingErrorType::InvalidIndex,
                message: format!("Invalid index {} (max: {})", index, max),
                transaction_hash: None,
                block_hash: None,
            },
            IndexingError::EmptyBlock { block_hash } => Self {
                error_type: IndexingErrorType::EmptyBlock,
                message: "Empty block".to_string(),
                transaction_hash: None,
                block_hash: Some(block_hash),
            },
            IndexingError::SerializationError { message } => Self {
                error_type: IndexingErrorType::SerializationError,
                message,
                transaction_hash: None,
                block_hash: None,
            },
            IndexingError::StorageError { message } => Self {
                error_type: IndexingErrorType::StorageError,
                message,
                transaction_hash: None,
                block_hash: None,
            },
            IndexingError::UnauthorizedSender {
                sender_id,
                expected,
            } => Self {
                error_type: IndexingErrorType::UnauthorizedSender,
                message: format!("Unauthorized sender {} (expected: {})", sender_id, expected),
                transaction_hash: None,
                block_hash: None,
            },
            IndexingError::CommunicationError { message } => Self {
                error_type: IndexingErrorType::CommunicationError,
                message,
                transaction_hash: None,
                block_hash: None,
            },
            IndexingError::Timeout { operation } => Self {
                error_type: IndexingErrorType::Timeout,
                message: format!("Operation timed out: {}", operation),
                transaction_hash: None,
                block_hash: None,
            },
        }
    }
}

/// Error type enumeration for IPC serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexingErrorType {
    TransactionNotFound,
    TreeNotCached,
    InvalidIndex,
    EmptyBlock,
    SerializationError,
    StorageError,
    UnauthorizedSender,
    CommunicationError,
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexing_error_display() {
        let err = IndexingError::InvalidIndex { index: 10, max: 5 };
        assert!(err.to_string().contains("10"));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn test_indexing_error_payload_from() {
        let tx_hash = [0x01; 32];
        let err = IndexingError::TransactionNotFound { tx_hash };
        let payload: IndexingErrorPayload = err.into();

        assert_eq!(payload.error_type, IndexingErrorType::TransactionNotFound);
        assert_eq!(payload.transaction_hash, Some(tx_hash));
    }
}
