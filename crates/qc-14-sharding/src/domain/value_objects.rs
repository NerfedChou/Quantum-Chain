//! # Domain Value Objects
//!
//! Immutable value types for Sharding.
//!
//! Reference: SPEC-14 Section 2.1 (Lines 54-101)

use super::errors::{Address, Hash, ShardId};
use serde::{Deserialize, Serialize};

/// Cross-shard transaction state machine.
/// Reference: SPEC-14 Lines 95-101
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CrossShardState {
    /// Initial state - transaction pending.
    #[default]
    Pending,
    /// Phase 1 complete - all locks acquired.
    Locked,
    /// Phase 2a complete - all shards committed.
    Committed,
    /// Phase 2b - transaction aborted/rolled back.
    Aborted,
}

impl CrossShardState {
    /// Check if transition to next state is valid.
    pub fn can_transition_to(&self, next: CrossShardState) -> bool {
        match (self, next) {
            (Self::Pending, Self::Locked) => true,
            (Self::Pending, Self::Aborted) => true, // Lock failed
            (Self::Locked, Self::Committed) => true,
            (Self::Locked, Self::Aborted) => true, // Commit failed or timeout
            _ => false,
        }
    }

    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Committed | Self::Aborted)
    }
}

/// Reason for transaction abort.
/// Reference: SPEC-14 Lines 374-379
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbortReason {
    /// Lock acquisition failed on a shard.
    LockFailed {
        /// Which shard failed
        shard_id: ShardId,
        /// Reason for failure
        reason: String,
    },
    /// Operation timed out.
    Timeout,
    /// Insufficient validator signatures.
    InsufficientSignatures,
    /// Explicit abort by coordinator.
    CoordinatorAbort,
    /// Network partition detected.
    NetworkPartition,
}

/// Shard assignment result.
/// Reference: SPEC-14 Lines 79-84
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardAssignment {
    /// The address being assigned.
    pub address: Address,
    /// The assigned shard.
    pub shard_id: ShardId,
    /// Epoch of assignment.
    pub epoch: u64,
}

impl ShardAssignment {
    /// Create a new shard assignment.
    pub fn new(address: Address, shard_id: ShardId, epoch: u64) -> Self {
        Self {
            address,
            shard_id,
            epoch,
        }
    }
}

/// Validator information.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Validator ID.
    pub id: [u8; 32],
    /// Stake amount.
    pub stake: u64,
    /// Assigned shard for current epoch.
    pub assigned_shard: ShardId,
}

/// Lock data for 2PC Phase 1.
/// Reference: SPEC-14 Lines 358-364
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockData {
    /// Transaction hash being locked.
    pub tx_hash: Hash,
    /// Shard holding the lock.
    pub shard_id: ShardId,
    /// Account being locked.
    pub account: Address,
    /// Amount being locked.
    pub amount: u64,
    /// Lock expiry timestamp.
    pub expires_at: u64,
}

/// Lock proof returned after successful lock.
/// Reference: SPEC-14 Lines 366-372
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockProof {
    /// Lock data.
    pub lock_data: LockData,
    /// Merkle proof of lock in shard state.
    pub merkle_proof: Vec<Hash>,
    /// Validator signatures.
    pub signatures: Vec<Signature>,
}

/// Validator signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// Validator ID.
    pub validator_id: [u8; 32],
    /// Signature bytes.
    pub signature_bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_shard_state_transitions_pending_to_locked() {
        assert!(CrossShardState::Pending.can_transition_to(CrossShardState::Locked));
    }

    #[test]
    fn test_cross_shard_state_transitions_pending_to_aborted() {
        assert!(CrossShardState::Pending.can_transition_to(CrossShardState::Aborted));
    }

    #[test]
    fn test_cross_shard_state_transitions_locked_to_committed() {
        assert!(CrossShardState::Locked.can_transition_to(CrossShardState::Committed));
    }

    #[test]
    fn test_cross_shard_state_transitions_locked_to_aborted() {
        assert!(CrossShardState::Locked.can_transition_to(CrossShardState::Aborted));
    }

    #[test]
    fn test_cross_shard_state_invalid_transition() {
        assert!(!CrossShardState::Committed.can_transition_to(CrossShardState::Pending));
        assert!(!CrossShardState::Aborted.can_transition_to(CrossShardState::Locked));
    }

    #[test]
    fn test_cross_shard_state_terminal() {
        assert!(CrossShardState::Committed.is_terminal());
        assert!(CrossShardState::Aborted.is_terminal());
        assert!(!CrossShardState::Pending.is_terminal());
        assert!(!CrossShardState::Locked.is_terminal());
    }

    #[test]
    fn test_shard_assignment() {
        let addr = [1u8; 20];
        let assignment = ShardAssignment::new(addr, 5, 100);
        assert_eq!(assignment.shard_id, 5);
        assert_eq!(assignment.epoch, 100);
    }

    #[test]
    fn test_abort_reason_lock_failed() {
        let reason = AbortReason::LockFailed {
            shard_id: 3,
            reason: "busy".to_string(),
        };
        match reason {
            AbortReason::LockFailed { shard_id, .. } => assert_eq!(shard_id, 3),
            _ => panic!("wrong variant"),
        }
    }
}
