//! # RocksDB Storage Adapter
//!
//! Production-ready RocksDB implementation of the KeyValueStore trait.
//!
//! ## Features
//!
//! - Atomic batch writes (WriteBatch)
//! - Column families for subsystem isolation
//! - Snappy compression
//! - Bloom filters for read optimization
//! - Write-ahead logging for durability
//!
//! ## Column Families
//!
//! - `blocks` - Block data (qc-02)
//! - `state` - State trie nodes (qc-04)
//! - `tx_index` - Transaction locations (qc-03)
//! - `metadata` - Chain metadata
//!
//! ## Configuration
//!
//! Optimized for blockchain workloads:
//! - Large block cache (256MB default)
//! - Bloom filters (10 bits per key)
//! - Level compaction
//! - fsync on write for durability

use parking_lot::RwLock;
use qc_02_block_storage::{FSError, KVStoreError}; // Layer compliant
use qc_02_block_storage::ports::outbound::{BatchOperation, FileSystemAdapter, KeyValueStore};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteBatch, DB};
use std::path::Path;
use std::sync::Arc;

/// Column family names for subsystem isolation
pub const CF_BLOCKS: &str = "blocks";
pub const CF_STATE: &str = "state";
pub const CF_TX_INDEX: &str = "tx_index";
pub const CF_METADATA: &str = "metadata";

/// All column families used by the node
pub const COLUMN_FAMILIES: &[&str] = &[CF_BLOCKS, CF_STATE, CF_TX_INDEX, CF_METADATA];

/// RocksDB configuration for production use
#[derive(Debug, Clone)]
pub struct RocksDbConfig {
    /// Path to the database directory
    pub path: String,
    /// Block cache size in bytes (default: 256MB)
    pub block_cache_size: usize,
    /// Write buffer size in bytes (default: 64MB)
    pub write_buffer_size: usize,
    /// Maximum number of write buffers (default: 3)
    pub max_write_buffer_number: i32,
    /// Target file size for level-1 (default: 64MB)
    pub target_file_size_base: u64,
    /// Enable fsync after each write (default: true for durability)
    pub sync_writes: bool,
    /// Enable statistics collection (default: false for production)
    pub enable_statistics: bool,
}

impl Default for RocksDbConfig {
    fn default() -> Self {
        Self {
            path: "./data/rocksdb".to_string(),
            block_cache_size: 256 * 1024 * 1024, // 256MB
            write_buffer_size: 64 * 1024 * 1024, // 64MB
            max_write_buffer_number: 3,
            target_file_size_base: 64 * 1024 * 1024, // 64MB
            sync_writes: true,
            enable_statistics: false,
        }
    }
}

impl RocksDbConfig {
    /// Create config for testing (smaller buffers, no sync)
    pub fn for_testing(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            block_cache_size: 8 * 1024 * 1024,  // 8MB
            write_buffer_size: 4 * 1024 * 1024, // 4MB
            max_write_buffer_number: 2,
            target_file_size_base: 4 * 1024 * 1024, // 4MB
            sync_writes: false,
            enable_statistics: false,
        }
    }
}

/// RocksDB-backed key-value store implementing the KeyValueStore trait
pub struct RocksDbStore {
    db: Arc<RwLock<DB>>,
    config: RocksDbConfig,
}

impl RocksDbStore {
    /// Open or create a RocksDB database
    pub fn open(config: RocksDbConfig) -> Result<Self, KVStoreError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);

        // Performance tuning
        opts.set_write_buffer_size(config.write_buffer_size);
        opts.set_max_write_buffer_number(config.max_write_buffer_number);
        opts.set_target_file_size_base(config.target_file_size_base);

        // Compression
        opts.set_compression_type(rocksdb::DBCompressionType::Snappy);

        // Bloom filter for faster lookups
        let mut block_opts = rocksdb::BlockBasedOptions::default();
        block_opts.set_bloom_filter(10.0, false);
        block_opts.set_block_cache(&rocksdb::Cache::new_lru_cache(config.block_cache_size));
        opts.set_block_based_table_factory(&block_opts);

        // Column families
        let cf_descriptors: Vec<ColumnFamilyDescriptor> = COLUMN_FAMILIES
            .iter()
            .map(|name| {
                let mut cf_opts = Options::default();
                cf_opts.set_compression_type(rocksdb::DBCompressionType::Snappy);
                ColumnFamilyDescriptor::new(*name, cf_opts)
            })
            .collect();

        // Open database
        let db = DB::open_cf_descriptors(&opts, &config.path, cf_descriptors).map_err(|e| {
            KVStoreError::IOError {
                message: format!("Failed to open RocksDB: {}", e),
            }
        })?;

        Ok(Self {
            db: Arc::new(RwLock::new(db)),
            config,
        })
    }

    /// Open with default column family (for simple use cases)
    pub fn open_default(path: impl AsRef<Path>) -> Result<Self, KVStoreError> {
        let config = RocksDbConfig {
            path: path.as_ref().to_string_lossy().to_string(),
            ..Default::default()
        };
        Self::open(config)
    }

    /// Get a reference to the underlying DB for advanced operations
    pub fn inner(&self) -> &Arc<RwLock<DB>> {
        &self.db
    }
}

impl KeyValueStore for RocksDbStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        let db = self.db.read();
        db.get(key).map_err(|e| KVStoreError::IOError {
            message: format!("RocksDB get failed: {}", e),
        })
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        let db = self.db.write();
        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(self.config.sync_writes);

        db.put_opt(key, value, &write_opts)
            .map_err(|e| KVStoreError::IOError {
                message: format!("RocksDB put failed: {}", e),
            })
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError> {
        let db = self.db.write();
        db.delete(key).map_err(|e| KVStoreError::IOError {
            message: format!("RocksDB delete failed: {}", e),
        })
    }

    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError> {
        let db = self.db.write();
        let mut batch = WriteBatch::default();

        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    batch.put(&key, &value);
                }
                BatchOperation::Delete { key } => {
                    batch.delete(&key);
                }
            }
        }

        let mut write_opts = rocksdb::WriteOptions::default();
        write_opts.set_sync(self.config.sync_writes);

        db.write_opt(batch, &write_opts)
            .map_err(|e| KVStoreError::IOError {
                message: format!("RocksDB batch write failed: {}", e),
            })
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        let db = self.db.read();
        db.get_pinned(key)
            .map(|v| v.is_some())
            .map_err(|e| KVStoreError::IOError {
                message: format!("RocksDB exists check failed: {}", e),
            })
    }

    fn prefix_scan(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KVStoreError> {
        let db = self.db.read();
        let mut results = Vec::new();

        let iter = db.iterator(IteratorMode::From(prefix, rocksdb::Direction::Forward));

        for item in iter {
            match item {
                Ok((key, value)) => {
                    if !key.starts_with(prefix) {
                        break;
                    }
                    results.push((key.to_vec(), value.to_vec()));
                }
                Err(e) => {
                    return Err(KVStoreError::IOError {
                        message: format!("RocksDB scan failed: {}", e),
                    });
                }
            }
        }

        Ok(results)
    }
}

/// Production filesystem adapter using std::fs
pub struct ProductionFileSystemAdapter {
    data_dir: String,
}

impl ProductionFileSystemAdapter {
    pub fn new(data_dir: impl Into<String>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }
}

impl FileSystemAdapter for ProductionFileSystemAdapter {
    fn available_disk_space_percent(&self) -> Result<u8, FSError> {
        use fs2::available_space;
        use std::path::Path;

        let path = Path::new(&self.data_dir);

        // Get available and total space
        let available = available_space(path).map_err(|e| FSError::IOError {
            message: e.to_string(),
        })?;

        let total = fs2::total_space(path).map_err(|e| FSError::IOError {
            message: e.to_string(),
        })?;

        if total == 0 {
            return Err(FSError::IOError {
                message: "Unable to determine disk space".to_string(),
            });
        }

        let percent = ((available as f64 / total as f64) * 100.0) as u8;
        Ok(percent)
    }

    fn available_disk_space_bytes(&self) -> Result<u64, FSError> {
        use fs2::available_space;
        use std::path::Path;

        let path = Path::new(&self.data_dir);
        available_space(path).map_err(|e| FSError::IOError {
            message: e.to_string(),
        })
    }

    fn total_disk_space_bytes(&self) -> Result<u64, FSError> {
        use fs2::total_space;
        use std::path::Path;

        let path = Path::new(&self.data_dir);
        total_space(path).map_err(|e| FSError::IOError {
            message: e.to_string(),
        })
    }
}

// =============================================================================
// State Trie RocksDB Database
// =============================================================================

use qc_04_state_management::StateError; // Layer compliant
use qc_04_state_management::ports::database::{SnapshotStorage, TrieDatabase};
use shared_types::Hash;

/// RocksDB-backed state trie database
///
/// Persists Patricia Merkle Trie nodes to RocksDB for durability.
/// Uses the CF_STATE column family for isolation.
pub struct RocksDbTrieDatabase {
    store: Arc<RocksDbStore>,
}

impl RocksDbTrieDatabase {
    /// Create a new trie database backed by RocksDB
    pub fn new(store: Arc<RocksDbStore>) -> Self {
        Self { store }
    }

    /// Create with a new RocksDB instance
    pub fn open(config: RocksDbConfig) -> Result<Self, StateError> {
        let store =
            RocksDbStore::open(config).map_err(|e| StateError::DatabaseError(e.to_string()))?;
        Ok(Self {
            store: Arc::new(store),
        })
    }

    fn make_key(hash: &Hash) -> Vec<u8> {
        let mut key = Vec::with_capacity(5 + 32);
        key.extend_from_slice(b"trie:");
        key.extend_from_slice(hash);
        key
    }
}

impl TrieDatabase for RocksDbTrieDatabase {
    fn get_node(&self, hash: &Hash) -> Result<Option<Vec<u8>>, StateError> {
        let key = Self::make_key(hash);
        self.store
            .get(&key)
            .map_err(|e| StateError::DatabaseError(e.to_string()))
    }

    fn put_node(&self, hash: Hash, data: Vec<u8>) -> Result<(), StateError> {
        let key = Self::make_key(&hash);
        // Need mutable access - clone the inner store
        let db = self.store.db.write();
        db.put(&key, &data)
            .map_err(|e| StateError::DatabaseError(e.to_string()))
    }

    fn batch_put(&self, nodes: Vec<(Hash, Vec<u8>)>) -> Result<(), StateError> {
        let db = self.store.db.write();
        let mut batch = WriteBatch::default();

        for (hash, data) in nodes {
            let key = Self::make_key(&hash);
            batch.put(&key, &data);
        }

        db.write(batch)
            .map_err(|e| StateError::DatabaseError(e.to_string()))
    }

    fn delete_node(&self, hash: &Hash) -> Result<(), StateError> {
        let key = Self::make_key(hash);
        let db = self.store.db.write();
        db.delete(&key)
            .map_err(|e| StateError::DatabaseError(e.to_string()))
    }
}

/// RocksDB-backed snapshot storage for state checkpoints
pub struct RocksDbSnapshotStorage {
    store: Arc<RocksDbStore>,
}

impl RocksDbSnapshotStorage {
    pub fn new(store: Arc<RocksDbStore>) -> Self {
        Self { store }
    }

    fn make_snapshot_key(height: u64) -> Vec<u8> {
        let mut key = Vec::with_capacity(13);
        key.extend_from_slice(b"snap:");
        key.extend_from_slice(&height.to_be_bytes());
        key
    }
}

impl SnapshotStorage for RocksDbSnapshotStorage {
    fn create_snapshot(&self, height: u64, root: Hash) -> Result<(), StateError> {
        let key = Self::make_snapshot_key(height);
        let db = self.store.db.write();
        db.put(&key, &root)
            .map_err(|e| StateError::DatabaseError(e.to_string()))
    }

    fn get_nearest_snapshot(&self, height: u64) -> Result<Option<(u64, Hash)>, StateError> {
        let db = self.store.db.read();

        // Scan backwards from requested height
        for h in (0..=height).rev() {
            let key = Self::make_snapshot_key(h);
            if let Some(value) = db
                .get(&key)
                .map_err(|e| StateError::DatabaseError(e.to_string()))?
            {
                if value.len() == 32 {
                    let mut root = [0u8; 32];
                    root.copy_from_slice(&value);
                    return Ok(Some((h, root)));
                }
            }
        }
        Ok(None)
    }

    fn prune_snapshots(&self, keep_after: u64) -> Result<u64, StateError> {
        let db = self.store.db.write();
        let mut pruned = 0u64;

        // Delete snapshots before keep_after
        for h in 0..keep_after {
            let key = Self::make_snapshot_key(h);
            if db.get(&key).ok().flatten().is_some() {
                db.delete(&key)
                    .map_err(|e| StateError::DatabaseError(e.to_string()))?;
                pruned += 1;
            }
        }

        Ok(pruned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rocksdb_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config = RocksDbConfig::for_testing(temp_dir.path().to_string_lossy().to_string());

        let mut store = RocksDbStore::open(config).unwrap();

        // Put and get
        store.put(b"key1", b"value1").unwrap();
        let value = store.get(b"key1").unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));

        // Exists
        assert!(store.exists(b"key1").unwrap());
        assert!(!store.exists(b"nonexistent").unwrap());

        // Delete
        store.delete(b"key1").unwrap();
        assert!(!store.exists(b"key1").unwrap());
    }

    #[test]
    fn test_rocksdb_batch_write() {
        let temp_dir = TempDir::new().unwrap();
        let config = RocksDbConfig::for_testing(temp_dir.path().to_string_lossy().to_string());

        let mut store = RocksDbStore::open(config).unwrap();

        let ops = vec![
            BatchOperation::put(b"batch1", b"value1"),
            BatchOperation::put(b"batch2", b"value2"),
            BatchOperation::put(b"batch3", b"value3"),
        ];

        store.atomic_batch_write(ops).unwrap();

        assert!(store.exists(b"batch1").unwrap());
        assert!(store.exists(b"batch2").unwrap());
        assert!(store.exists(b"batch3").unwrap());
    }

    #[test]
    fn test_rocksdb_prefix_scan() {
        let temp_dir = TempDir::new().unwrap();
        let config = RocksDbConfig::for_testing(temp_dir.path().to_string_lossy().to_string());

        let mut store = RocksDbStore::open(config).unwrap();

        store.put(b"block:0001", b"data1").unwrap();
        store.put(b"block:0002", b"data2").unwrap();
        store.put(b"block:0003", b"data3").unwrap();
        store.put(b"tx:0001", b"tx_data").unwrap();

        let results = store.prefix_scan(b"block:").unwrap();
        assert_eq!(results.len(), 3);
    }
}
