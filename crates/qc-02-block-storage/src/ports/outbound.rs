//! # Outbound Ports (Driven Ports)
//!
//! Dependencies required by the Block Storage service.
//!
//! ## SPEC-02 Section 3.2
//!
//! These are the interfaces this library requires the host application to implement.

use crate::domain::errors::{FSError, KVStoreError, SerializationError};
use crate::domain::storage::{StoredBlock, Timestamp};

/// Type alias for key-value scan results to simplify complex return types.
pub type ScanResult = Vec<(Vec<u8>, Vec<u8>)>;

/// Abstract interface for key-value database operations.
///
/// Production: `RocksDbStore` (node-runtime/adapters/storage/rocksdb_adapter.rs)
/// Testing: `InMemoryKVStore` (below)
///
/// Reference: SPEC-02 Section 3.2 (Driven Ports)
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError>;

    /// Put a single key-value pair.
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError>;

    /// Delete a key.
    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError>;

    /// Execute an atomic batch write.
    ///
    /// ## Atomicity Guarantee (INVARIANT-4)
    ///
    /// Either ALL operations in the batch succeed, or NONE are applied.
    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError>;

    /// Check if a key exists.
    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError>;

    /// Iterate over keys with a prefix.
    fn prefix_scan(&self, prefix: &[u8]) -> Result<ScanResult, KVStoreError>;
}

/// Batch operation for atomic writes.
#[derive(Debug, Clone)]
pub enum BatchOperation {
    /// Put a key-value pair.
    Put { key: Vec<u8>, value: Vec<u8> },
    /// Delete a key.
    Delete { key: Vec<u8> },
}

impl BatchOperation {
    /// Create a Put operation.
    pub fn put(key: impl Into<Vec<u8>>, value: impl Into<Vec<u8>>) -> Self {
        BatchOperation::Put {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Create a Delete operation.
    pub fn delete(key: impl Into<Vec<u8>>) -> Self {
        BatchOperation::Delete { key: key.into() }
    }
}

/// Abstract interface for filesystem operations.
///
/// Used to check disk space before writes (INVARIANT-2).
pub trait FileSystemAdapter: Send + Sync {
    /// Get available disk space as a percentage (0-100).
    fn available_disk_space_percent(&self) -> Result<u8, FSError>;

    /// Get available disk space in bytes.
    fn available_disk_space_bytes(&self) -> Result<u64, FSError>;

    /// Get total disk space in bytes.
    fn total_disk_space_bytes(&self) -> Result<u64, FSError>;
}

/// Abstract interface for checksum computation.
///
/// Used for data integrity verification (INVARIANT-3).
pub trait ChecksumProvider: Send + Sync {
    /// Compute CRC32C checksum of data.
    fn compute_crc32c(&self, data: &[u8]) -> u32;

    /// Verify CRC32C checksum matches.
    fn verify_crc32c(&self, data: &[u8], expected: u32) -> bool {
        self.compute_crc32c(data) == expected
    }
}

/// Abstract interface for time operations (for testability).
pub trait TimeSource: Send + Sync {
    /// Get current timestamp in seconds since epoch.
    fn now(&self) -> Timestamp;
}

/// Abstract interface for block serialization.
pub trait BlockSerializer: Send + Sync {
    /// Serialize a StoredBlock to bytes.
    fn serialize(&self, block: &StoredBlock) -> Result<Vec<u8>, SerializationError>;

    /// Deserialize bytes to a StoredBlock.
    fn deserialize(&self, data: &[u8]) -> Result<StoredBlock, SerializationError>;

    /// Estimate the serialized size of a block (for size limit checks).
    fn estimate_size(&self, block: &StoredBlock) -> usize;
}

// =============================================================================
// ADAPTER IMPLEMENTATIONS
// =============================================================================
// Implementations have been moved to crate::adapters::*
// - adapters::storage::InMemoryKVStore
// - adapters::storage::FileBackedKVStore
// - adapters::filesystem::MockFileSystemAdapter
// - adapters::infra::DefaultChecksumProvider
// - adapters::infra::SystemTimeSource
// - adapters::serializer::BincodeBlockSerializer
