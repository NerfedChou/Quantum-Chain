//! # Handler Types
//!
//! Error types and conversions for IPC handlers.

use crate::domain::errors::StorageError;
use crate::ipc::envelope::EnvelopeError;
use crate::ipc::payloads::{StorageErrorPayload, StorageErrorType};

/// Handler error types
#[derive(Debug)]
pub enum HandlerError {
    /// Envelope validation failed
    Envelope(EnvelopeError),
    /// Storage operation failed
    Storage(StorageError),
}

impl From<EnvelopeError> for HandlerError {
    fn from(e: EnvelopeError) -> Self {
        HandlerError::Envelope(e)
    }
}

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Envelope(e) => write!(f, "Envelope error: {}", e),
            Self::Storage(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl std::error::Error for HandlerError {}

/// Convert StorageError to StorageErrorPayload
pub fn storage_error_to_payload(e: StorageError) -> StorageErrorPayload {
    match e {
        StorageError::BlockNotFound { hash } => StorageErrorPayload {
            error_type: StorageErrorType::BlockNotFound,
            message: format!("Block not found: {:?}", hash),
            block_hash: Some(hash),
            block_height: None,
        },
        StorageError::HeightNotFound { height } => StorageErrorPayload {
            error_type: StorageErrorType::HeightNotFound,
            message: format!("Height not found: {}", height),
            block_hash: None,
            block_height: Some(height),
        },
        StorageError::TransactionNotFound { tx_hash } => StorageErrorPayload {
            error_type: StorageErrorType::TransactionNotFound,
            message: format!("Transaction not found: {:?}", tx_hash),
            block_hash: None,
            block_height: None,
        },
        StorageError::DataCorruption {
            block_hash,
            expected_checksum,
            actual_checksum,
        } => StorageErrorPayload {
            error_type: StorageErrorType::DataCorruption,
            message: format!(
                "Checksum mismatch: expected {}, got {}",
                expected_checksum, actual_checksum
            ),
            block_hash: Some(block_hash),
            block_height: None,
        },
        StorageError::DiskFull {
            available_percent, ..
        } => StorageErrorPayload {
            error_type: StorageErrorType::DiskFull,
            message: format!("Disk full: {}% available", available_percent),
            block_hash: None,
            block_height: None,
        },
        StorageError::UnauthorizedSender {
            sender_id,
            expected_id,
            operation,
        } => StorageErrorPayload {
            error_type: StorageErrorType::UnauthorizedSender,
            message: format!(
                "Unauthorized sender {} for {}: expected {}",
                sender_id, operation, expected_id
            ),
            block_hash: None,
            block_height: None,
        },
        _ => StorageErrorPayload {
            error_type: StorageErrorType::DatabaseError,
            message: format!("{}", e),
            block_hash: None,
            block_height: None,
        },
    }
}
