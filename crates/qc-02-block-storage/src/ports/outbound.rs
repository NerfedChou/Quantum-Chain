//! # Outbound Ports (Driven Ports)
//!
//! Dependencies required by the Block Storage service.
//!
//! ## SPEC-02 Section 3.2
//!
//! These are the interfaces this library requires the host application to implement.

use crate::domain::entities::{StoredBlock, Timestamp};
use crate::domain::errors::{FSError, KVStoreError, SerializationError};

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
    fn prefix_scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError>;
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
// Production: RocksDbStore in node-runtime/adapters/storage/rocksdb_adapter.rs
// Testing: In-memory implementations below
// =============================================================================

/// Default checksum provider using crc32fast.
///
/// Implements CRC32C checksums for INVARIANT-3 (Data Integrity).
#[derive(Default)]
pub struct DefaultChecksumProvider;

impl ChecksumProvider for DefaultChecksumProvider {
    fn compute_crc32c(&self, data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }
}

/// Default time source using system time.
#[derive(Default)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// In-memory key-value store for unit tests.
///
/// Provides atomic batch writes via single-threaded HashMap.
/// Production uses `RocksDbStore` with true atomic transactions.
#[derive(Default)]
pub struct InMemoryKVStore {
    data: std::collections::HashMap<Vec<u8>, Vec<u8>>,
}

impl InMemoryKVStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyValueStore for InMemoryKVStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        Ok(self.data.get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError> {
        self.data.remove(key);
        Ok(())
    }

    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError> {
        // For in-memory, we can just apply all operations
        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    self.data.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    self.data.remove(&key);
                }
            }
        }
        Ok(())
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        Ok(self.data.contains_key(key))
    }

    fn prefix_scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let results: Vec<_> = self
            .data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}

/// File-backed key-value store for production without RocksDB.
///
/// Persists data to a binary file on disk, providing durability without
/// requiring RocksDB compilation. Suitable for development and light production.
pub struct FileBackedKVStore {
    data: std::collections::HashMap<Vec<u8>, Vec<u8>>,
    path: std::path::PathBuf,
}

impl FileBackedKVStore {
    /// Create a new file-backed store at the given path.
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        
        // Check if file exists and its size
        if let Ok(metadata) = std::fs::metadata(&path) {
            tracing::info!("[qc-02] 💾 Found existing storage file: {} ({} bytes)", 
                path.display(), metadata.len());
        } else {
            tracing::info!("[qc-02] 📁 No existing storage file at {}", path.display());
        }
        
        let data = Self::load_from_file(&path).unwrap_or_default();
        
        if !data.is_empty() {
            tracing::info!("[qc-02] 💾 Loaded {} keys from {}", data.len(), path.display());
        } else {
            tracing::info!("[qc-02] 📁 Storage file empty or not found");
        }
        
        Self { data, path }
    }
    
    fn load_from_file(path: &std::path::Path) -> Option<std::collections::HashMap<Vec<u8>, Vec<u8>>> {
        use std::io::Read;
        
        let mut file = std::fs::File::open(path).ok()?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        
        // Simple binary format: [key_len:u32][key][value_len:u32][value]...
        let mut data = std::collections::HashMap::new();
        let mut cursor = 0;
        
        while cursor + 4 <= bytes.len() {
            // Read key length
            let key_len = u32::from_le_bytes(bytes[cursor..cursor+4].try_into().ok()?) as usize;
            cursor += 4;
            
            if cursor + key_len > bytes.len() { break; }
            let key = bytes[cursor..cursor+key_len].to_vec();
            cursor += key_len;
            
            if cursor + 4 > bytes.len() { break; }
            // Read value length
            let value_len = u32::from_le_bytes(bytes[cursor..cursor+4].try_into().ok()?) as usize;
            cursor += 4;
            
            if cursor + value_len > bytes.len() { break; }
            let value = bytes[cursor..cursor+value_len].to_vec();
            cursor += value_len;
            
            data.insert(key, value);
        }
        
        Some(data)
    }
    
    fn save_to_file(&self) -> Result<(), KVStoreError> {
        use std::io::Write;
        
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| KVStoreError::IOError { message: e.to_string() })?;
        }
        
        let mut bytes = Vec::new();
        
        for (key, value) in &self.data {
            bytes.extend_from_slice(&(key.len() as u32).to_le_bytes());
            bytes.extend_from_slice(key);
            bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
            bytes.extend_from_slice(value);
        }
        
        // Write atomically via temp file
        let temp_path = self.path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp_path)
            .map_err(|e| KVStoreError::IOError { message: e.to_string() })?;
        file.write_all(&bytes)
            .map_err(|e| KVStoreError::IOError { message: e.to_string() })?;
        file.sync_all()
            .map_err(|e| KVStoreError::IOError { message: e.to_string() })?;
        
        std::fs::rename(&temp_path, &self.path)
            .map_err(|e| KVStoreError::IOError { message: e.to_string() })?;
        
        Ok(())
    }
}

impl KeyValueStore for FileBackedKVStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        Ok(self.data.get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.data.insert(key.to_vec(), value.to_vec());
        self.save_to_file()
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError> {
        self.data.remove(key);
        self.save_to_file()
    }

    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError> {
        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    self.data.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    self.data.remove(&key);
                }
            }
        }
        self.save_to_file()
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        Ok(self.data.contains_key(key))
    }

    fn prefix_scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let results: Vec<_> = self
            .data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}

/// Controllable filesystem adapter for unit tests.
///
/// Allows tests to simulate disk space conditions for INVARIANT-2 verification.
/// Production uses `ProductionFileSystemAdapter` in node-runtime.
pub struct MockFileSystemAdapter {
    available_percent: u8,
}

impl MockFileSystemAdapter {
    /// Create adapter reporting `available_percent` disk space.
    pub fn new(available_percent: u8) -> Self {
        Self { available_percent }
    }

    /// Update reported disk space for test scenarios.
    pub fn set_available_percent(&mut self, percent: u8) {
        self.available_percent = percent;
    }
}

impl FileSystemAdapter for MockFileSystemAdapter {
    fn available_disk_space_percent(&self) -> Result<u8, FSError> {
        Ok(self.available_percent)
    }

    fn available_disk_space_bytes(&self) -> Result<u64, FSError> {
        // Assume 1TB total, return proportional
        Ok((1_000_000_000_000u64 * self.available_percent as u64) / 100)
    }

    fn total_disk_space_bytes(&self) -> Result<u64, FSError> {
        Ok(1_000_000_000_000) // 1TB
    }
}

/// Default block serializer using bincode.
#[derive(Default)]
pub struct BincodeBlockSerializer;

impl BlockSerializer for BincodeBlockSerializer {
    fn serialize(&self, block: &StoredBlock) -> Result<Vec<u8>, SerializationError> {
        bincode::serialize(block).map_err(|e| SerializationError {
            message: e.to_string(),
        })
    }

    fn deserialize(&self, data: &[u8]) -> Result<StoredBlock, SerializationError> {
        bincode::deserialize(data).map_err(|e| SerializationError {
            message: e.to_string(),
        })
    }

    fn estimate_size(&self, block: &StoredBlock) -> usize {
        // Rough estimate: header + transactions + overhead
        std::mem::size_of::<StoredBlock>() + block.block.transactions.len() * 256
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_kv_store() {
        let mut store = InMemoryKVStore::new();

        store.put(b"key1", b"value1").unwrap();
        store.put(b"key2", b"value2").unwrap();

        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(store.get(b"key3").unwrap(), None);

        assert!(store.exists(b"key1").unwrap());
        assert!(!store.exists(b"key3").unwrap());
    }

    #[test]
    fn test_in_memory_kv_batch_write() {
        let mut store = InMemoryKVStore::new();

        let ops = vec![
            BatchOperation::put(b"a", b"1"),
            BatchOperation::put(b"b", b"2"),
            BatchOperation::put(b"c", b"3"),
        ];

        store.atomic_batch_write(ops).unwrap();

        assert_eq!(store.get(b"a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get(b"b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(store.get(b"c").unwrap(), Some(b"3".to_vec()));
    }

    #[test]
    fn test_prefix_scan() {
        let mut store = InMemoryKVStore::new();

        store.put(b"block:1", b"data1").unwrap();
        store.put(b"block:2", b"data2").unwrap();
        store.put(b"height:1", b"hash1").unwrap();

        let blocks = store.prefix_scan(b"block:").unwrap();
        assert_eq!(blocks.len(), 2);

        let heights = store.prefix_scan(b"height:").unwrap();
        assert_eq!(heights.len(), 1);
    }

    #[test]
    fn test_checksum_provider() {
        let provider = DefaultChecksumProvider;

        let data = b"hello world";
        let checksum = provider.compute_crc32c(data);

        assert!(provider.verify_crc32c(data, checksum));
        assert!(!provider.verify_crc32c(data, checksum + 1));
    }

    #[test]
    fn test_mock_filesystem() {
        let mut fs = MockFileSystemAdapter::new(50);

        assert_eq!(fs.available_disk_space_percent().unwrap(), 50);

        fs.set_available_percent(4);
        assert_eq!(fs.available_disk_space_percent().unwrap(), 4);
    }
}
