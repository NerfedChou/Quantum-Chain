//! # Domain Value Objects
//!
//! Immutable value types for Light Client Sync.
//!
//! Reference: SPEC-13 Section 2.1 (Lines 74-124)

use serde::{Deserialize, Serialize};
use super::errors::Hash;

/// Checkpoint source type.
/// Reference: SPEC-13 Lines 82-87
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CheckpointSource {
    /// Built-in hardcoded checkpoint (genesis, mainnet forks).
    Hardcoded,
    /// Verified by multi-node consensus.
    MultiNodeConsensus {
        /// Number of nodes that agreed
        node_count: usize,
    },
    /// External trusted source (e.g., block explorer API).
    External {
        /// Source URL or identifier
        source: String,
    },
}

/// Trusted checkpoint for chain validation.
/// Reference: SPEC-13 Lines 75-80
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Checkpoint {
    /// Block height of this checkpoint.
    pub height: u64,
    /// Block hash at this height.
    pub hash: Hash,
    /// Source of this checkpoint.
    pub source: CheckpointSource,
}

impl Checkpoint {
    /// Create a new hardcoded checkpoint.
    pub fn hardcoded(height: u64, hash: Hash) -> Self {
        Self {
            height,
            hash,
            source: CheckpointSource::Hardcoded,
        }
    }

    /// Create a checkpoint from multi-node consensus.
    pub fn from_consensus(height: u64, hash: Hash, node_count: usize) -> Self {
        Self {
            height,
            hash,
            source: CheckpointSource::MultiNodeConsensus { node_count },
        }
    }
}

/// Current chain tip information.
/// Reference: SPEC-13 Lines 211-216
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChainTip {
    /// Current tip block hash.
    pub hash: Hash,
    /// Current tip block height.
    pub height: u64,
    /// Number of confirmations.
    pub confirmations: u64,
}

impl ChainTip {
    /// Create a new chain tip.
    pub fn new(hash: Hash, height: u64) -> Self {
        Self {
            hash,
            height,
            confirmations: 0,
        }
    }

    /// Create chain tip with confirmations.
    pub fn with_confirmations(hash: Hash, height: u64, confirmations: u64) -> Self {
        Self {
            hash,
            height,
            confirmations,
        }
    }
}

/// Result of a sync operation.
/// Reference: SPEC-13 Lines 201-208
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncResult {
    /// Was the sync successful?
    pub success: bool,
    /// Number of headers synced.
    pub headers_synced: u64,
    /// Current chain tip after sync.
    pub tip: ChainTip,
    /// Time taken in milliseconds.
    pub duration_ms: u64,
}

impl SyncResult {
    /// Create a successful sync result.
    pub fn success(headers_synced: u64, tip: ChainTip, duration_ms: u64) -> Self {
        Self {
            success: true,
            headers_synced,
            tip,
            duration_ms,
        }
    }

    /// Create a failed sync result.
    pub fn failed(tip: ChainTip, duration_ms: u64) -> Self {
        Self {
            success: false,
            headers_synced: 0,
            tip,
            duration_ms,
        }
    }
}

/// Position in Merkle proof (left or right sibling).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Position {
    /// Sibling is on the left.
    Left,
    /// Sibling is on the right.
    Right,
}

/// Node in a Merkle proof path.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProofNode {
    /// Hash of the sibling node.
    pub hash: Hash,
    /// Position of the sibling.
    pub position: Position,
}

impl ProofNode {
    /// Create a left sibling node.
    pub fn left(hash: Hash) -> Self {
        Self {
            hash,
            position: Position::Left,
        }
    }

    /// Create a right sibling node.
    pub fn right(hash: Hash) -> Self {
        Self {
            hash,
            position: Position::Right,
        }
    }
}

/// Merkle proof for transaction inclusion.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerkleProof {
    /// Transaction hash being proven.
    pub tx_hash: Hash,
    /// Proof path from leaf to root.
    pub path: Vec<ProofNode>,
    /// Expected Merkle root (from block header).
    pub merkle_root: Hash,
    /// Block hash containing this transaction.
    pub block_hash: Hash,
    /// Block height.
    pub block_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_hardcoded() {
        let hash = [1u8; 32];
        let cp = Checkpoint::hardcoded(100000, hash);
        assert_eq!(cp.height, 100000);
        assert_eq!(cp.source, CheckpointSource::Hardcoded);
    }

    #[test]
    fn test_checkpoint_consensus() {
        let hash = [2u8; 32];
        let cp = Checkpoint::from_consensus(200000, hash, 5);
        assert_eq!(cp.height, 200000);
        match cp.source {
            CheckpointSource::MultiNodeConsensus { node_count } => assert_eq!(node_count, 5),
            _ => panic!("wrong source type"),
        }
    }

    #[test]
    fn test_chain_tip_new() {
        let hash = [3u8; 32];
        let tip = ChainTip::new(hash, 500000);
        assert_eq!(tip.height, 500000);
        assert_eq!(tip.confirmations, 0);
    }

    #[test]
    fn test_chain_tip_with_confirmations() {
        let hash = [4u8; 32];
        let tip = ChainTip::with_confirmations(hash, 600000, 6);
        assert_eq!(tip.confirmations, 6);
    }

    #[test]
    fn test_sync_result_success() {
        let tip = ChainTip::new([5u8; 32], 700000);
        let result = SyncResult::success(1000, tip, 2500);
        assert!(result.success);
        assert_eq!(result.headers_synced, 1000);
    }

    #[test]
    fn test_sync_result_failed() {
        let tip = ChainTip::new([6u8; 32], 700000);
        let result = SyncResult::failed(tip, 500);
        assert!(!result.success);
        assert_eq!(result.headers_synced, 0);
    }

    #[test]
    fn test_proof_node_positions() {
        let left = ProofNode::left([7u8; 32]);
        let right = ProofNode::right([8u8; 32]);
        assert_eq!(left.position, Position::Left);
        assert_eq!(right.position, Position::Right);
    }
}
