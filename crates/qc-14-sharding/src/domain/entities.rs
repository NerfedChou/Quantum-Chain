//! # Domain Entities
//!
//! Core entities for Sharding subsystem.
//!
//! Reference: SPEC-14 Section 2.1 (Lines 58-117)

use serde::{Deserialize, Serialize};
use super::errors::{ShardId, Hash, Address, ShardError};
use super::value_objects::{CrossShardState, AbortReason, LockProof};

/// Shard configuration.
/// Reference: SPEC-14 Lines 58-77
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardConfig {
    /// Total number of shards.
    pub shard_count: u16,
    /// Current epoch.
    pub epoch: u64,
    /// Minimum validators per shard.
    /// Reference: System.md Line 700
    pub min_validators_per_shard: usize,
    /// Cross-shard transaction timeout in seconds.
    /// Reference: SPEC-14 Lines 650-652
    pub cross_shard_timeout_secs: u64,
    /// Required signature threshold (2/3 = 0.67).
    pub signature_threshold: f64,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self {
            shard_count: 16,
            epoch: 0,
            min_validators_per_shard: 128,
            cross_shard_timeout_secs: 30,
            signature_threshold: 0.67,
        }
    }
}

impl ShardConfig {
    /// Create config for testing.
    pub fn for_testing() -> Self {
        Self {
            shard_count: 4,
            epoch: 0,
            min_validators_per_shard: 4,
            cross_shard_timeout_secs: 5,
            signature_threshold: 0.67,
        }
    }
}

/// Cross-shard transaction.
/// Reference: SPEC-14 Lines 86-93
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossShardTransaction {
    /// Transaction hash (unique identifier).
    pub tx_hash: Hash,
    /// Source shard ID.
    pub source_shard: ShardId,
    /// Target shard IDs.
    pub target_shards: Vec<ShardId>,
    /// Sender address.
    pub sender: Address,
    /// Recipient addresses (one per target shard).
    pub recipients: Vec<Address>,
    /// Amount being transferred.
    pub amount: u64,
    /// Current state in 2PC.
    pub state: CrossShardState,
    /// Lock proofs from Phase 1.
    pub lock_proofs: Vec<LockProof>,
    /// Abort reason if aborted.
    pub abort_reason: Option<AbortReason>,
    /// Creation timestamp.
    pub created_at: u64,
}

impl CrossShardTransaction {
    /// Create a new pending cross-shard transaction.
    pub fn new(
        tx_hash: Hash,
        source_shard: ShardId,
        target_shards: Vec<ShardId>,
        sender: Address,
        recipients: Vec<Address>,
        amount: u64,
        created_at: u64,
    ) -> Self {
        Self {
            tx_hash,
            source_shard,
            target_shards,
            sender,
            recipients,
            amount,
            state: CrossShardState::Pending,
            lock_proofs: Vec::new(),
            abort_reason: None,
            created_at,
        }
    }

    /// Transition to a new state.
    pub fn transition_to(&mut self, new_state: CrossShardState) -> Result<(), ShardError> {
        if !self.state.can_transition_to(new_state) {
            return Err(ShardError::InvalidTransition {
                from: format!("{:?}", self.state),
                to: format!("{:?}", new_state),
            });
        }
        self.state = new_state;
        Ok(())
    }

    /// Abort with reason.
    pub fn abort(&mut self, reason: AbortReason) -> Result<(), ShardError> {
        self.abort_reason = Some(reason);
        self.transition_to(CrossShardState::Aborted)
    }

    /// Add a lock proof.
    pub fn add_lock_proof(&mut self, proof: LockProof) {
        self.lock_proofs.push(proof);
    }

    /// Check if all locks are acquired.
    pub fn all_locks_acquired(&self) -> bool {
        self.lock_proofs.len() == self.target_shards.len() + 1 // +1 for source
    }
}

/// Shard state root.
/// Reference: SPEC-14 Lines 103-109
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardStateRoot {
    /// Shard ID.
    pub shard_id: ShardId,
    /// State root hash.
    pub state_root: Hash,
    /// Block height.
    pub block_height: u64,
    /// Epoch.
    pub epoch: u64,
}

impl ShardStateRoot {
    /// Create a new shard state root.
    pub fn new(shard_id: ShardId, state_root: Hash, block_height: u64, epoch: u64) -> Self {
        Self {
            shard_id,
            state_root,
            block_height,
            epoch,
        }
    }
}

/// Global state root (aggregate of all shard roots).
/// Reference: SPEC-14 Lines 111-117
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalStateRoot {
    /// Combined Merkle root of all shard roots.
    pub root: Hash,
    /// Individual shard roots.
    pub shard_roots: Vec<ShardStateRoot>,
    /// Block height.
    pub block_height: u64,
    /// Epoch.
    pub epoch: u64,
}

impl GlobalStateRoot {
    /// Create a new global state root.
    pub fn new(root: Hash, shard_roots: Vec<ShardStateRoot>, block_height: u64, epoch: u64) -> Self {
        Self {
            root,
            shard_roots,
            block_height,
            epoch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tx() -> CrossShardTransaction {
        CrossShardTransaction::new(
            [1u8; 32],
            0,
            vec![1, 2],
            [10u8; 20],
            vec![[11u8; 20], [12u8; 20]],
            1000,
            12345,
        )
    }

    #[test]
    fn test_shard_config_default() {
        let config = ShardConfig::default();
        assert_eq!(config.shard_count, 16);
        assert_eq!(config.min_validators_per_shard, 128);
        assert_eq!(config.cross_shard_timeout_secs, 30);
    }

    #[test]
    fn test_cross_shard_transaction_new() {
        let tx = create_test_tx();
        assert_eq!(tx.state, CrossShardState::Pending);
        assert!(tx.lock_proofs.is_empty());
        assert!(tx.abort_reason.is_none());
    }

    #[test]
    fn test_cross_shard_transition_pending_to_locked() {
        let mut tx = create_test_tx();
        assert!(tx.transition_to(CrossShardState::Locked).is_ok());
        assert_eq!(tx.state, CrossShardState::Locked);
    }

    #[test]
    fn test_cross_shard_transition_invalid() {
        let mut tx = create_test_tx();
        tx.state = CrossShardState::Committed;
        assert!(tx.transition_to(CrossShardState::Pending).is_err());
    }

    #[test]
    fn test_cross_shard_abort() {
        let mut tx = create_test_tx();
        tx.abort(AbortReason::Timeout).unwrap();
        assert_eq!(tx.state, CrossShardState::Aborted);
        assert!(matches!(tx.abort_reason, Some(AbortReason::Timeout)));
    }

    #[test]
    fn test_shard_state_root() {
        let root = ShardStateRoot::new(0, [1u8; 32], 100, 10);
        assert_eq!(root.shard_id, 0);
        assert_eq!(root.block_height, 100);
    }

    #[test]
    fn test_global_state_root() {
        let shard_roots = vec![
            ShardStateRoot::new(0, [1u8; 32], 100, 10),
            ShardStateRoot::new(1, [2u8; 32], 100, 10),
        ];
        let global = GlobalStateRoot::new([3u8; 32], shard_roots, 100, 10);
        assert_eq!(global.shard_roots.len(), 2);
    }
}
