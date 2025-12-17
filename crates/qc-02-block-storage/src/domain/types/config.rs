//! # Value Objects
//!
//! Immutable configuration and value types for the Block Storage subsystem.
//!
//! ## SPEC-02 Reference
//!
//! - Section 2.5: StorageConfig, KeyPrefix, CompactionStrategy

use crate::domain::assembler::AssemblyConfig;
use shared_types::Hash;

/// Configuration for the storage engine.
///
/// ## SPEC-02 Section 2.5
///
/// All configuration values have sensible defaults for production use.
///
/// ## Security Note
///
/// Checksum verification is ALWAYS enabled and cannot be disabled.
/// This is a compile-time guarantee per Gemini security audit (2025-12-03).
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Minimum required disk space percentage (default: 5%).
    ///
    /// INVARIANT-2: Writes fail if disk space falls below this threshold.
    pub min_disk_space_percent: u8,

    /// Maximum block size in bytes (default: 10MB).
    pub max_block_size: usize,

    /// Compaction strategy for the underlying KV store.
    pub compaction_strategy: CompactionStrategy,

    /// Assembly buffer configuration (V2.3 Choreography).
    pub assembly_config: AssemblyConfig,

    /// Whether to persist the transaction index to disk (default: false).
    ///
    /// When `false` (default): Transaction index is in-memory only.
    /// - Fast O(1) lookups
    /// - Suitable for development and light nodes
    /// - Index lost on restart (rebuilt on demand)
    ///
    /// When `true`: Transaction index is persisted to KV store.
    /// - Survives restarts
    /// - Required for production nodes with large transaction volumes
    /// - Uses prefix `t:{tx_hash} -> TransactionLocation`
    pub persist_transaction_index: bool,
}

impl StorageConfig {
    /// Checksum verification is ALWAYS enabled (compile-time guarantee).
    ///
    /// INVARIANT-3: Data integrity is non-negotiable.
    #[inline]
    pub const fn verify_checksums(&self) -> bool {
        true
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            min_disk_space_percent: 5,
            max_block_size: 10 * 1024 * 1024, // 10 MB
            compaction_strategy: CompactionStrategy::LeveledCompaction,
            assembly_config: AssemblyConfig::default(),
            persist_transaction_index: false, // Default: in-memory only
        }
    }
}

impl StorageConfig {
    /// Create a new configuration with custom values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the minimum disk space percentage.
    pub fn with_min_disk_space(mut self, percent: u8) -> Self {
        self.min_disk_space_percent = percent;
        self
    }

    /// Set the maximum block size.
    pub fn with_max_block_size(mut self, size: usize) -> Self {
        self.max_block_size = size;
        self
    }

    /// Set the assembly configuration.
    pub fn with_assembly_config(mut self, config: AssemblyConfig) -> Self {
        self.assembly_config = config;
        self
    }

    /// Enable or disable transaction index persistence.
    ///
    /// When enabled, the transaction index is persisted to the KV store,
    /// surviving restarts. Recommended for production nodes with large
    /// transaction volumes.
    pub fn with_persist_transaction_index(mut self, persist: bool) -> Self {
        self.persist_transaction_index = persist;
        self
    }
}

/// Compaction strategy for the LSM tree backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStrategy {
    /// Leveled compaction (default, good for reads).
    LeveledCompaction,
    /// Size-tiered compaction (good for writes).
    SizeTieredCompaction,
}

/// Key prefixes for the key-value store.
///
/// All keys are prefixed to namespace different data types.
#[derive(Debug, Clone, Copy)]
pub enum KeyPrefix {
    /// Block data: `b:{hash}` -> StoredBlock
    Block,
    /// Height to hash index: `h:{height}` -> Hash
    BlockByHeight,
    /// Storage metadata: `m:metadata` -> StorageMetadata
    Metadata,
    /// Transaction index: `t:{tx_hash}` -> TransactionLocation
    Transaction,
}

impl KeyPrefix {
    /// Get the byte prefix for this key type.
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            KeyPrefix::Block => b"b:",
            KeyPrefix::BlockByHeight => b"h:",
            KeyPrefix::Metadata => b"m:",
            KeyPrefix::Transaction => b"t:",
        }
    }

    /// Build a full key with the given suffix.
    pub fn key(&self, suffix: &[u8]) -> Vec<u8> {
        let mut key = self.as_bytes().to_vec();
        key.extend_from_slice(suffix);
        key
    }

    /// Build a block key from a hash.
    pub fn block_key(hash: &Hash) -> Vec<u8> {
        KeyPrefix::Block.key(hash)
    }

    /// Build a height key from a block height.
    pub fn height_key(height: u64) -> Vec<u8> {
        KeyPrefix::BlockByHeight.key(&height.to_be_bytes())
    }

    /// Build a transaction key from a hash.
    pub fn transaction_key(tx_hash: &Hash) -> Vec<u8> {
        KeyPrefix::Transaction.key(tx_hash)
    }

    /// Get the metadata key.
    pub fn metadata_key() -> Vec<u8> {
        KeyPrefix::Metadata.key(b"metadata")
    }
}

/// Location of a transaction within a stored block.
///
/// ## SPEC-02 Section 3.1 (V2.3)
///
/// Used by Transaction Indexing for Merkle proof generation.
///
/// ## Persistence
///
/// When `StorageConfig::persist_transaction_index` is enabled, this struct
/// is serialized to the KV store with key prefix `t:{tx_hash}`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TransactionLocation {
    /// Hash of the block containing this transaction.
    pub block_hash: Hash,
    /// Height of the block containing this transaction.
    pub block_height: u64,
    /// Index of the transaction within the block's transaction list.
    pub transaction_index: usize,
    /// Cached Merkle root (for efficient proof generation).
    pub merkle_root: Hash,
}

impl TransactionLocation {
    /// Create a new transaction location.
    pub fn new(
        block_hash: Hash,
        block_height: u64,
        transaction_index: usize,
        merkle_root: Hash,
    ) -> Self {
        Self {
            block_hash,
            block_height,
            transaction_index,
            merkle_root,
        }
    }
}
