//! # Event Payloads
//!
//! Incoming event payloads per IPC-MATRIX.md.

use shared_types::{Hash, ValidatedBlock};

/// BlockValidated event from Consensus (Subsystem 8)
///
/// Block Storage buffers this until MerkleRootComputed and StateRootComputed arrive.
#[derive(Debug, Clone)]
pub struct BlockValidatedPayload {
    /// The consensus-validated block
    pub block: ValidatedBlock,
    /// Block hash for correlation with other events
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
}

/// MerkleRootComputed event from Transaction Indexing (Subsystem 3)
///
/// Block Storage buffers this until BlockValidated and StateRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The computed Merkle root of transactions
    pub merkle_root: Hash,
}

/// StateRootComputed event from State Management (Subsystem 4)
///
/// Block Storage buffers this until BlockValidated and MerkleRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The state root after executing this block
    pub state_root: Hash,
}
