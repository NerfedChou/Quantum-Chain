//! # Domain Entities
//!
//! Core domain entities for the Block Storage subsystem.
//!
//! ## SPEC-02 Reference
//!
//! - Section 2.2: StoredBlock, BlockIndex, StorageMetadata
//! - Section 2.3: Index Structures

use serde::{Deserialize, Serialize};
use shared_types::{Hash, ValidatedBlock};

/// Unix timestamp in seconds since epoch.
pub type Timestamp = u64;

/// A block stored on disk with integrity checksum.
///
/// This is the storage-layer wrapper around ValidatedBlock.
/// It adds storage-specific metadata (timestamp, checksum) that
/// are not part of the consensus-validated block structure.
///
/// ## SPEC-02 Section 2.2
///
/// The checksum is computed over (block + merkle_root + state_root) at write time
/// and verified on every read operation (INVARIANT-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBlock {
    /// The complete block data (from shared-types).
    pub block: ValidatedBlock,
    /// Merkle root of transactions (from Tx Indexing via Event Bus).
    pub merkle_root: Hash,
    /// State root after block execution (from State Mgmt via Event Bus).
    pub state_root: Hash,
    /// Timestamp when block was stored (local storage time, not block time).
    pub stored_at: Timestamp,
    /// CRC32C checksum computed at write time for integrity verification.
    pub checksum: u32,
}

impl StoredBlock {
    /// Create a new stored block with checksum computed.
    ///
    /// The checksum is computed over the serialized block data + roots.
    pub fn new(
        block: ValidatedBlock,
        merkle_root: Hash,
        state_root: Hash,
        stored_at: Timestamp,
        checksum: u32,
    ) -> Self {
        Self {
            block,
            merkle_root,
            state_root,
            stored_at,
            checksum,
        }
    }

    /// Get the block hash (from the header).
    pub fn block_hash(&self) -> Hash {
        // Compute hash from header fields
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.block.header.parent_hash);
        hasher.update(self.block.header.height.to_le_bytes());
        hasher.update(self.block.header.merkle_root);
        hasher.update(self.block.header.state_root);
        hasher.update(self.block.header.timestamp.to_le_bytes());
        hasher.finalize().into()
    }

    /// Get the block height.
    pub fn height(&self) -> u64 {
        self.block.header.height
    }

    /// Get the parent hash.
    pub fn parent_hash(&self) -> Hash {
        self.block.header.parent_hash
    }
}

/// Mapping from block height to block hash.
///
/// Stored separately for O(1) height-based lookups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockIndex {
    /// Index entries (sorted by height).
    entries: Vec<BlockIndexEntry>,
}

impl BlockIndex {
    /// Create a new empty index.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Add an entry to the index.
    pub fn insert(&mut self, height: u64, block_hash: Hash) {
        // Keep sorted by height
        let entry = BlockIndexEntry { height, block_hash };
        match self.entries.binary_search_by_key(&height, |e| e.height) {
            Ok(pos) => self.entries[pos] = entry,        // Update existing
            Err(pos) => self.entries.insert(pos, entry), // Insert at correct position
        }
    }

    /// Get the block hash at a given height.
    pub fn get(&self, height: u64) -> Option<Hash> {
        self.entries
            .binary_search_by_key(&height, |e| e.height)
            .ok()
            .map(|pos| self.entries[pos].block_hash)
    }

    /// Check if height exists in index.
    pub fn contains(&self, height: u64) -> bool {
        self.entries
            .binary_search_by_key(&height, |e| e.height)
            .is_ok()
    }

    /// Get the latest height in the index.
    pub fn latest_height(&self) -> Option<u64> {
        self.entries.last().map(|e| e.height)
    }

    /// Get total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A single entry in the block index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockIndexEntry {
    /// Block height.
    pub height: u64,
    /// Block hash at this height.
    pub block_hash: Hash,
}

/// Global storage metadata.
///
/// Tracks the overall state of the storage subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetadata {
    /// Hash of the genesis block (immutable after first write - INVARIANT-6).
    pub genesis_hash: Option<Hash>,
    /// Height of the latest stored block.
    pub latest_height: u64,
    /// Height of the latest finalized block (monotonic - INVARIANT-5).
    pub finalized_height: u64,
    /// Total number of blocks stored.
    pub total_blocks: u64,
    /// Storage format version for migrations.
    pub storage_version: u16,
}

impl Default for StorageMetadata {
    fn default() -> Self {
        Self {
            genesis_hash: None,
            latest_height: 0,
            finalized_height: 0,
            total_blocks: 0,
            storage_version: 1,
        }
    }
}

impl StorageMetadata {
    /// Create new metadata with genesis block.
    pub fn with_genesis(genesis_hash: Hash) -> Self {
        Self {
            genesis_hash: Some(genesis_hash),
            latest_height: 0,
            finalized_height: 0,
            total_blocks: 1,
            storage_version: 1,
        }
    }

    /// Update metadata after storing a new block.
    pub fn on_block_stored(&mut self, height: u64, block_hash: Hash) {
        if self.genesis_hash.is_none() && height == 0 {
            self.genesis_hash = Some(block_hash);
        }
        if height > self.latest_height {
            self.latest_height = height;
        }
        self.total_blocks += 1;
    }

    /// Update metadata after finalization.
    ///
    /// Returns `false` if finalization would violate monotonicity (INVARIANT-5).
    pub fn on_finalized(&mut self, height: u64) -> bool {
        if height <= self.finalized_height {
            return false;
        }
        self.finalized_height = height;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_index_insert_and_get() {
        let mut index = BlockIndex::new();

        index.insert(0, [0x00; 32]);
        index.insert(1, [0x01; 32]);
        index.insert(2, [0x02; 32]);

        assert_eq!(index.get(0), Some([0x00; 32]));
        assert_eq!(index.get(1), Some([0x01; 32]));
        assert_eq!(index.get(2), Some([0x02; 32]));
        assert_eq!(index.get(3), None);
    }

    #[test]
    fn test_block_index_maintains_order() {
        let mut index = BlockIndex::new();

        // Insert out of order
        index.insert(5, [0x05; 32]);
        index.insert(1, [0x01; 32]);
        index.insert(3, [0x03; 32]);

        assert_eq!(index.latest_height(), Some(5));
        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_storage_metadata_genesis_immutability() {
        let mut meta = StorageMetadata::default();

        // First block at height 0 sets genesis
        meta.on_block_stored(0, [0x01; 32]);
        assert_eq!(meta.genesis_hash, Some([0x01; 32]));

        // Subsequent height 0 blocks don't change genesis
        meta.on_block_stored(0, [0x02; 32]);
        assert_eq!(meta.genesis_hash, Some([0x01; 32]));
    }

    #[test]
    fn test_storage_metadata_finalization_monotonicity() {
        let mut meta = StorageMetadata::default();

        // Finalize height 5
        assert!(meta.on_finalized(5));
        assert_eq!(meta.finalized_height, 5);

        // Cannot regress to height 3
        assert!(!meta.on_finalized(3));
        assert_eq!(meta.finalized_height, 5);

        // Cannot re-finalize same height
        assert!(!meta.on_finalized(5));
        assert_eq!(meta.finalized_height, 5);

        // Can finalize higher
        assert!(meta.on_finalized(7));
        assert_eq!(meta.finalized_height, 7);
    }
}
