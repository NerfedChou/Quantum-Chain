//! # Block Index
//!
//! Mapping from block height to block hash for O(1) lookups.
//!
//! ## SPEC-02 Section 2.3

use serde::{Deserialize, Serialize};
use shared_types::Hash;

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
