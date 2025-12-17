//! # Pending Block Assembly
//!
//! Represents a partial block assembly awaiting completion.
//!
//! ## SPEC-02 Section 2.4
//!
//! Tracks which of the three required components have arrived from the
//! choreography pattern (BlockValidated, MerkleRootComputed, StateRootComputed).

use crate::domain::storage::Timestamp;
use shared_types::{Hash, ValidatedBlock};

/// A partial block assembly awaiting completion.
///
/// Tracks which of the three required components have arrived.
#[derive(Debug, Clone)]
pub struct PendingBlockAssembly {
    /// Block hash (key for this assembly).
    pub block_hash: Hash,
    /// Block height for ordering.
    pub block_height: u64,
    /// When this assembly was first started (for timeout).
    pub started_at: Timestamp,
    /// The validated block (from Consensus, Subsystem 8).
    pub validated_block: Option<ValidatedBlock>,
    /// Merkle root of transactions (from Tx Indexing, Subsystem 3).
    pub merkle_root: Option<Hash>,
    /// State root after execution (from State Management, Subsystem 4).
    pub state_root: Option<Hash>,
}

impl PendingBlockAssembly {
    /// Create a new empty pending assembly.
    pub fn new(block_hash: Hash, started_at: Timestamp) -> Self {
        Self {
            block_hash,
            block_height: 0,
            started_at,
            validated_block: None,
            merkle_root: None,
            state_root: None,
        }
    }

    /// Check if all three components are present.
    pub fn is_complete(&self) -> bool {
        self.validated_block.is_some() && self.merkle_root.is_some() && self.state_root.is_some()
    }

    /// Check if this assembly has timed out.
    pub fn is_expired(&self, now: Timestamp, timeout_secs: u64) -> bool {
        now.saturating_sub(self.started_at) > timeout_secs
    }

    /// Get the components as a tuple if complete.
    ///
    /// Returns `None` if not all components are present.
    pub fn take_components(self) -> Option<(ValidatedBlock, Hash, Hash)> {
        match (self.validated_block, self.merkle_root, self.state_root) {
            (Some(block), Some(merkle), Some(state)) => Some((block, merkle, state)),
            _ => None,
        }
    }

    /// Get the age of this assembly in seconds.
    pub fn age(&self, now: Timestamp) -> u64 {
        now.saturating_sub(self.started_at)
    }
}
