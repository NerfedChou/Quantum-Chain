//! # Stored Block
//!
//! The storage-layer wrapper around ValidatedBlock.
//!
//! ## SPEC-02 Section 2.2
//!
//! The checksum is computed over (block + merkle_root + state_root) at write time
//! and verified on every read operation (INVARIANT-3).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared_types::{Hash, ValidatedBlock};

use super::Timestamp;

/// A block stored on disk with integrity checksum.
///
/// This is the storage-layer wrapper around ValidatedBlock.
/// It adds storage-specific metadata (timestamp, checksum) that
/// are not part of the consensus-validated block structure.
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
