//! # Domain Errors
//!
//! Error types for the Block Storage subsystem.
//!
//! ## Design Principles
//!
//! - Each error maps to a specific domain invariant violation
//! - Errors are descriptive and actionable
//! - No panics in domain logic (use Result instead)

use shared_types::Hash;
use std::fmt;

/// Errors that can occur during storage operations.
///
/// Each variant corresponds to a specific invariant violation or failure mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Block with this hash was not found.
    BlockNotFound { hash: Hash },

    /// No block exists at this height.
    HeightNotFound { height: u64 },

    /// Block with this hash already exists.
    BlockExists { hash: Hash },

    /// Parent block not found (INVARIANT-1 violation).
    ParentNotFound { parent_hash: Hash },

    /// Disk space below minimum threshold (INVARIANT-2 violation).
    DiskFull {
        available_percent: u8,
        required_percent: u8,
    },

    /// Checksum mismatch detected (INVARIANT-3 violation).
    DataCorruption {
        block_hash: Hash,
        expected_checksum: u32,
        actual_checksum: u32,
    },

    /// Block exceeds maximum size limit.
    BlockTooLarge { size: usize, max_size: usize },

    /// Finalization height invalid (INVARIANT-5 violation).
    InvalidFinalization { requested: u64, current: u64 },

    /// Genesis block cannot be modified (INVARIANT-6 violation).
    GenesisImmutable,

    /// Transaction not found in any stored block.
    TransactionNotFound { tx_hash: Hash },

    /// Assembly timeout - incomplete block purged (INVARIANT-7).
    AssemblyTimeout {
        block_hash: Hash,
        pending_duration_secs: u64,
    },

    /// Database I/O error.
    DatabaseError { message: String },

    /// Serialization/deserialization error.
    SerializationError { message: String },

    /// Unauthorized sender for this operation.
    UnauthorizedSender {
        sender_id: u8,
        expected_id: u8,
        operation: &'static str,
    },

    /// Non-canonical encoding detected (Anti-Malleability defense).
    ///
    /// Input bytes differ from re-serialized canonical form.
    /// Accepting non-canonical data could cause hash mismatches with network.
    NonCanonicalEncoding { reason: &'static str },

    /// Database lock could not be acquired (process already running).
    DatabaseLocked { message: String },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::BlockNotFound { hash } => {
                write!(f, "Block not found: {:02x?}...", &hash[..4])
            }
            StorageError::HeightNotFound { height } => {
                write!(f, "No block at height {}", height)
            }
            StorageError::BlockExists { hash } => {
                write!(f, "Block already exists: {:02x?}...", &hash[..4])
            }
            StorageError::ParentNotFound { parent_hash } => {
                write!(
                    f,
                    "Parent block not found: {:02x?}... (INVARIANT-1)",
                    &parent_hash[..4]
                )
            }
            StorageError::DiskFull {
                available_percent,
                required_percent,
            } => {
                write!(
                    f,
                    "Disk space critical: {}% available, {}% required (INVARIANT-2)",
                    available_percent, required_percent
                )
            }
            StorageError::DataCorruption {
                block_hash,
                expected_checksum,
                actual_checksum,
            } => {
                write!(f, "Data corruption detected for block {:02x?}...: expected checksum {}, got {} (INVARIANT-3)", 
                    &block_hash[..4], expected_checksum, actual_checksum)
            }
            StorageError::BlockTooLarge { size, max_size } => {
                write!(f, "Block too large: {} bytes, max {} bytes", size, max_size)
            }
            StorageError::InvalidFinalization { requested, current } => {
                write!(f, "Invalid finalization: cannot finalize height {} when current is {} (INVARIANT-5)", 
                    requested, current)
            }
            StorageError::GenesisImmutable => {
                write!(f, "Genesis block is immutable (INVARIANT-6)")
            }
            StorageError::TransactionNotFound { tx_hash } => {
                write!(f, "Transaction not found: {:02x?}...", &tx_hash[..4])
            }
            StorageError::AssemblyTimeout {
                block_hash,
                pending_duration_secs,
            } => {
                write!(
                    f,
                    "Assembly timeout for block {:02x?}... after {}s (INVARIANT-7)",
                    &block_hash[..4],
                    pending_duration_secs
                )
            }
            StorageError::DatabaseError { message } => {
                write!(f, "Database error: {}", message)
            }
            StorageError::SerializationError { message } => {
                write!(f, "Serialization error: {}", message)
            }
            StorageError::UnauthorizedSender {
                sender_id,
                expected_id,
                operation,
            } => {
                write!(
                    f,
                    "Unauthorized sender {} for {}: expected subsystem {}",
                    sender_id, operation, expected_id
                )
            }
            StorageError::NonCanonicalEncoding { reason } => {
                write!(
                    f,
                    "Non-canonical encoding detected: {} (Anti-Malleability)",
                    reason
                )
            }
            StorageError::DatabaseLocked { message } => {
                write!(f, "Database locked: {}", message)
            }
        }
    }
}

impl std::error::Error for StorageError {}

/// Key-value store errors.
#[derive(Debug, Clone)]
pub enum KVStoreError {
    /// I/O error during read/write.
    IOError { message: String },
    /// Data corruption in the store.
    CorruptionError { message: String },
    /// Key not found.
    NotFound,
}

impl fmt::Display for KVStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KVStoreError::IOError { message } => write!(f, "KV store I/O error: {}", message),
            KVStoreError::CorruptionError { message } => {
                write!(f, "KV store corruption: {}", message)
            }
            KVStoreError::NotFound => write!(f, "Key not found in KV store"),
        }
    }
}

impl std::error::Error for KVStoreError {}

impl From<KVStoreError> for StorageError {
    fn from(err: KVStoreError) -> Self {
        StorageError::DatabaseError {
            message: err.to_string(),
        }
    }
}

/// Filesystem adapter errors.
#[derive(Debug, Clone)]
pub enum FSError {
    /// I/O error.
    IOError { message: String },
    /// Permission denied.
    PermissionDenied,
}

impl fmt::Display for FSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FSError::IOError { message } => write!(f, "Filesystem I/O error: {}", message),
            FSError::PermissionDenied => write!(f, "Filesystem permission denied"),
        }
    }
}

impl std::error::Error for FSError {}

/// Serialization errors.
#[derive(Debug, Clone)]
pub struct SerializationError {
    pub message: String,
}

impl fmt::Display for SerializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Serialization error: {}", self.message)
    }
}

impl std::error::Error for SerializationError {}

impl From<SerializationError> for StorageError {
    fn from(err: SerializationError) -> Self {
        StorageError::SerializationError {
            message: err.message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = StorageError::ParentNotFound {
            parent_hash: [0xAB; 32],
        };
        let msg = format!("{}", err);
        assert!(msg.contains("INVARIANT-1"));
        assert!(msg.contains("Parent block not found"));
    }

    #[test]
    fn test_kv_error_conversion() {
        let kv_err = KVStoreError::IOError {
            message: "disk failure".to_string(),
        };
        let storage_err: StorageError = kv_err.into();

        match storage_err {
            StorageError::DatabaseError { message } => {
                assert!(message.contains("disk failure"));
            }
            _ => panic!("Expected DatabaseError"),
        }
    }
}
