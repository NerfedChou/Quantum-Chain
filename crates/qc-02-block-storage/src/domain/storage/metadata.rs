//! # Storage Metadata
//!
//! Global storage metadata tracking the overall state.
//!
//! ## SPEC-02 Section 2.3

use serde::{Deserialize, Serialize};
use shared_types::Hash;

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
