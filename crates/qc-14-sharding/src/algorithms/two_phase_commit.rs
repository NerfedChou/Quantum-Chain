//! # Two-Phase Commit Protocol
//!
//! 2PC coordinator for cross-shard transactions.
//!
//! Reference: System.md Line 681, SPEC-14 Lines 626-655

use std::time::Instant;
use crate::domain::{
    ShardId, Hash, ShardError, CrossShardState,
    CrossShardTransaction, LockProof, AbortReason,
};

/// 2PC coordinator state.
#[derive(Clone, Debug)]
pub struct TwoPhaseCoordinator {
    /// Transaction being coordinated.
    pub tx_hash: Hash,
    /// Source shard.
    pub source_shard: ShardId,
    /// Target shards.
    pub target_shards: Vec<ShardId>,
    /// Current state.
    pub state: CrossShardState,
    /// Lock proofs received.
    pub lock_proofs: Vec<LockProof>,
    /// Timeout in seconds.
    pub timeout_secs: u64,
    /// Start time.
    start_time: Instant,
}

impl TwoPhaseCoordinator {
    /// Create a new 2PC coordinator.
    pub fn new(
        tx_hash: Hash,
        source_shard: ShardId,
        target_shards: Vec<ShardId>,
        timeout_secs: u64,
    ) -> Self {
        Self {
            tx_hash,
            source_shard,
            target_shards,
            state: CrossShardState::Pending,
            lock_proofs: Vec::new(),
            timeout_secs,
            start_time: Instant::now(),
        }
    }

    /// Check if operation has timed out.
    pub fn is_timeout(&self) -> bool {
        self.start_time.elapsed().as_secs() > self.timeout_secs
    }

    /// Add a lock proof from a shard.
    pub fn add_lock_proof(&mut self, proof: LockProof) -> Result<(), ShardError> {
        if self.is_timeout() {
            return Err(ShardError::Timeout(self.timeout_secs));
        }

        if self.state != CrossShardState::Pending {
            return Err(ShardError::InvalidTransition {
                from: format!("{:?}", self.state),
                to: "receiving locks".to_string(),
            });
        }

        self.lock_proofs.push(proof);
        Ok(())
    }

    /// Check if all locks are acquired.
    pub fn all_locks_acquired(&self) -> bool {
        // Need locks from source + all targets
        self.lock_proofs.len() >= self.target_shards.len() + 1
    }

    /// Try to transition to locked state.
    pub fn try_lock(&mut self) -> Result<(), ShardError> {
        if self.is_timeout() {
            self.state = CrossShardState::Aborted;
            return Err(ShardError::Timeout(self.timeout_secs));
        }

        if !self.all_locks_acquired() {
            return Err(ShardError::LockFailed(format!(
                "Only {} of {} locks acquired",
                self.lock_proofs.len(),
                self.target_shards.len() + 1
            )));
        }

        self.state = CrossShardState::Locked;
        Ok(())
    }

    /// Commit the transaction.
    pub fn commit(&mut self) -> Result<(), ShardError> {
        if self.state != CrossShardState::Locked {
            return Err(ShardError::InvalidTransition {
                from: format!("{:?}", self.state),
                to: "Committed".to_string(),
            });
        }

        self.state = CrossShardState::Committed;
        Ok(())
    }

    /// Abort the transaction.
    pub fn abort(&mut self, reason: AbortReason) -> Result<AbortReason, ShardError> {
        if self.state.is_terminal() {
            return Err(ShardError::InvalidTransition {
                from: format!("{:?}", self.state),
                to: "Aborted".to_string(),
            });
        }

        self.state = CrossShardState::Aborted;
        Ok(reason)
    }

    /// Get current state.
    pub fn current_state(&self) -> CrossShardState {
        self.state
    }

    /// Get elapsed time.
    pub fn elapsed_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

/// Determine final outcome of 2PC.
pub fn decide_outcome(
    lock_responses: &[Result<LockProof, ShardError>],
    required_locks: usize,
) -> Result<Vec<LockProof>, AbortReason> {
    let successes: Vec<_> = lock_responses
        .iter()
        .filter_map(|r| r.as_ref().ok().cloned())
        .collect();

    if successes.len() >= required_locks {
        Ok(successes)
    } else {
        // Find first error
        let first_error = lock_responses
            .iter()
            .find_map(|r| r.as_ref().err());

        match first_error {
            Some(ShardError::Timeout(_)) => Err(AbortReason::Timeout),
            Some(ShardError::LockFailed(reason)) => Err(AbortReason::LockFailed {
                shard_id: 0, // Would need to track which shard
                reason: reason.clone(),
            }),
            _ => Err(AbortReason::CoordinatorAbort),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::LockData;

    fn create_coordinator() -> TwoPhaseCoordinator {
        TwoPhaseCoordinator::new([1u8; 32], 0, vec![1, 2], 30)
    }

    fn create_lock_proof(shard: ShardId) -> LockProof {
        LockProof {
            lock_data: LockData {
                tx_hash: [1u8; 32],
                shard_id: shard,
                account: [0u8; 20],
                amount: 100,
                expires_at: 0,
            },
            merkle_proof: vec![],
            signatures: vec![],
        }
    }

    #[test]
    fn test_coordinator_new() {
        let coord = create_coordinator();
        assert_eq!(coord.state, CrossShardState::Pending);
        assert!(coord.lock_proofs.is_empty());
    }

    #[test]
    fn test_coordinator_add_lock_proof() {
        let mut coord = create_coordinator();
        coord.add_lock_proof(create_lock_proof(0)).unwrap();
        assert_eq!(coord.lock_proofs.len(), 1);
    }

    #[test]
    fn test_coordinator_all_locks_acquired() {
        let mut coord = create_coordinator();
        // Need 3 locks: source (0) + targets (1, 2)
        assert!(!coord.all_locks_acquired());

        coord.add_lock_proof(create_lock_proof(0)).unwrap();
        coord.add_lock_proof(create_lock_proof(1)).unwrap();
        coord.add_lock_proof(create_lock_proof(2)).unwrap();

        assert!(coord.all_locks_acquired());
    }

    #[test]
    fn test_coordinator_try_lock_success() {
        let mut coord = create_coordinator();
        coord.add_lock_proof(create_lock_proof(0)).unwrap();
        coord.add_lock_proof(create_lock_proof(1)).unwrap();
        coord.add_lock_proof(create_lock_proof(2)).unwrap();

        assert!(coord.try_lock().is_ok());
        assert_eq!(coord.state, CrossShardState::Locked);
    }

    #[test]
    fn test_coordinator_try_lock_insufficient() {
        let mut coord = create_coordinator();
        coord.add_lock_proof(create_lock_proof(0)).unwrap();

        assert!(coord.try_lock().is_err());
        assert_eq!(coord.state, CrossShardState::Pending);
    }

    #[test]
    fn test_coordinator_commit() {
        let mut coord = create_coordinator();
        coord.add_lock_proof(create_lock_proof(0)).unwrap();
        coord.add_lock_proof(create_lock_proof(1)).unwrap();
        coord.add_lock_proof(create_lock_proof(2)).unwrap();
        coord.try_lock().unwrap();

        assert!(coord.commit().is_ok());
        assert_eq!(coord.state, CrossShardState::Committed);
    }

    #[test]
    fn test_coordinator_commit_from_pending_fails() {
        let mut coord = create_coordinator();
        assert!(coord.commit().is_err());
    }

    #[test]
    fn test_coordinator_abort() {
        let mut coord = create_coordinator();
        let reason = coord.abort(AbortReason::Timeout).unwrap();
        assert!(matches!(reason, AbortReason::Timeout));
        assert_eq!(coord.state, CrossShardState::Aborted);
    }

    #[test]
    fn test_decide_outcome_success() {
        let responses: Vec<Result<LockProof, ShardError>> = vec![
            Ok(create_lock_proof(0)),
            Ok(create_lock_proof(1)),
            Ok(create_lock_proof(2)),
        ];

        let result = decide_outcome(&responses, 3);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_decide_outcome_failure() {
        let responses: Vec<Result<LockProof, ShardError>> = vec![
            Ok(create_lock_proof(0)),
            Err(ShardError::Timeout(30)),
        ];

        let result = decide_outcome(&responses, 3);
        assert!(result.is_err());
    }
}
