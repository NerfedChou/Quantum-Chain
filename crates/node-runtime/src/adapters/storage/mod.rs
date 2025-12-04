//! # Production Storage Adapters
//!
//! Production-ready storage backends using RocksDB.
//!
//! ## Usage
//!
//! Enable the `rocksdb` feature to use these adapters:
//!
//! ```toml
//! node-runtime = { path = "...", features = ["rocksdb"] }
//! ```
//!
//! ## Architecture
//!
//! RocksDB is used for:
//! - Block storage (qc-02)
//! - State trie persistence (qc-04)
//! - Transaction index (qc-03)
//!
//! Each subsystem gets its own column family for isolation.

#[cfg(feature = "rocksdb")]
pub mod rocksdb_adapter;

#[cfg(feature = "rocksdb")]
pub use rocksdb_adapter::{
    RocksDbStore, RocksDbConfig, ProductionFileSystemAdapter,
    RocksDbTrieDatabase, RocksDbSnapshotStorage,
    CF_BLOCKS, CF_STATE, CF_TX_INDEX, CF_METADATA, COLUMN_FAMILIES,
};

// Re-export in-memory adapters for testing
pub use qc_02_block_storage::ports::outbound::{
    InMemoryKVStore, MockFileSystemAdapter, DefaultChecksumProvider, SystemTimeSource,
    BincodeBlockSerializer,
};
