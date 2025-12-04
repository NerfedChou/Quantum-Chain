//! Checkpoint entity
//!
//! Reference: SPEC-09-FINALITY.md Section 2.1

use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// Unique identifier for a checkpoint
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId {
    pub epoch: u64,
    pub block_hash: Hash,
}

impl CheckpointId {
    pub fn new(epoch: u64, block_hash: Hash) -> Self {
        Self { epoch, block_hash }
    }
}

/// Checkpoint finality state
///
/// Reference: SPEC-09-FINALITY.md Section 2.1
/// State progression: Pending → Justified → Finalized
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[derive(Default)]
pub enum CheckpointState {
    /// Not yet justified - awaiting attestations
    #[default]
    Pending,
    /// 2/3+ validators attested - justified but not finalized
    Justified,
    /// Two consecutive justified checkpoints - economically final
    Finalized,
}


/// A finality checkpoint at an epoch boundary
///
/// Reference: SPEC-09-FINALITY.md Section 2.1
/// Checkpoints occur at epoch boundaries (every `epoch_length` blocks)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Epoch number (checkpoint identifier)
    pub epoch: u64,
    /// Block hash at epoch boundary
    pub block_hash: Hash,
    /// Block height at epoch boundary
    pub block_height: u64,
    /// Current state of this checkpoint
    pub state: CheckpointState,
    /// Total stake that attested (for verification)
    pub attested_stake: u128,
    /// Total active stake at this epoch
    pub total_stake: u128,
}

impl Checkpoint {
    /// Create a new pending checkpoint
    pub fn new(epoch: u64, block_hash: Hash, block_height: u64) -> Self {
        Self {
            epoch,
            block_hash,
            block_height,
            state: CheckpointState::Pending,
            attested_stake: 0,
            total_stake: 0,
        }
    }

    /// Create checkpoint with total stake info
    pub fn with_total_stake(mut self, total_stake: u128) -> Self {
        self.total_stake = total_stake;
        self
    }

    /// Get checkpoint ID
    pub fn id(&self) -> CheckpointId {
        CheckpointId::new(self.epoch, self.block_hash)
    }

    /// Check if checkpoint is justified
    pub fn is_justified(&self) -> bool {
        self.state >= CheckpointState::Justified
    }

    /// Check if checkpoint is finalized
    pub fn is_finalized(&self) -> bool {
        self.state == CheckpointState::Finalized
    }

    /// Add attested stake and check justification threshold
    ///
    /// INVARIANT-2: Justification requires >= 2/3 of total stake
    /// Reference: SPEC-09-FINALITY.md Section 2.2
    pub fn add_attestation_stake(&mut self, stake: u128) -> bool {
        self.attested_stake = self.attested_stake.saturating_add(stake);
        self.check_justification_threshold()
    }

    /// Check if justification threshold is met
    ///
    /// INVARIANT-2: 2/3 = 67% threshold
    /// 
    /// SECURITY: Uses checked arithmetic to prevent overflow attacks.
    /// If total_stake is extremely large, we use saturating arithmetic
    /// to prevent incorrect threshold calculations.
    pub fn check_justification_threshold(&self) -> bool {
        if self.total_stake == 0 {
            return false;
        }
        // 2/3 + 1 for strict majority
        // Use checked arithmetic to prevent overflow when total_stake > u128::MAX / 2
        let required = self.total_stake
            .checked_mul(2)
            .map(|v| v / 3 + 1)
            .unwrap_or_else(|| {
                // Overflow case: total_stake is huge, use saturating division
                // (total_stake / 3) * 2 + 1 avoids overflow
                (self.total_stake / 3).saturating_mul(2).saturating_add(1)
            });
        self.attested_stake >= required
    }

    /// Justify this checkpoint if threshold met
    /// Returns true if state changed
    pub fn try_justify(&mut self) -> bool {
        if self.state == CheckpointState::Pending && self.check_justification_threshold() {
            self.state = CheckpointState::Justified;
            true
        } else {
            false
        }
    }

    /// Finalize this checkpoint
    ///
    /// INVARIANT-1: Can only finalize if already justified
    /// Reference: SPEC-09-FINALITY.md Section 2.2
    pub fn finalize(&mut self) -> bool {
        if self.state == CheckpointState::Justified {
            self.state = CheckpointState::Finalized;
            true
        } else {
            false
        }
    }

    /// Calculate participation percentage (for metrics)
    pub fn participation_percent(&self) -> u8 {
        if self.total_stake == 0 {
            return 0;
        }
        ((self.attested_stake * 100) / self.total_stake) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(n: u8) -> Hash {
        let mut hash = [0u8; 32];
        hash[0] = n;
        hash
    }

    #[test]
    fn test_checkpoint_state_ordering() {
        assert!(CheckpointState::Pending < CheckpointState::Justified);
        assert!(CheckpointState::Justified < CheckpointState::Finalized);
    }

    #[test]
    fn test_justification_at_67_percent() {
        let mut cp = Checkpoint::new(1, test_hash(1), 32).with_total_stake(100);

        // Add 66% - not enough
        cp.add_attestation_stake(66);
        assert!(!cp.is_justified());

        // Add 1 more to reach 67% - now justified
        cp.add_attestation_stake(1);
        cp.try_justify();
        assert!(cp.is_justified());
    }

    #[test]
    fn test_justification_below_threshold() {
        let mut cp = Checkpoint::new(1, test_hash(1), 32).with_total_stake(100);

        cp.add_attestation_stake(66);
        cp.try_justify();
        assert!(!cp.is_justified());
    }

    #[test]
    fn test_finalization_requires_justification() {
        let mut cp = Checkpoint::new(1, test_hash(1), 32);

        // Cannot finalize pending checkpoint
        assert!(!cp.finalize());
        assert_eq!(cp.state, CheckpointState::Pending);

        // Force justify
        cp.state = CheckpointState::Justified;

        // Now can finalize
        assert!(cp.finalize());
        assert_eq!(cp.state, CheckpointState::Finalized);
    }

    #[test]
    fn test_participation_percent() {
        let mut cp = Checkpoint::new(1, test_hash(1), 32).with_total_stake(100);

        cp.attested_stake = 75;
        assert_eq!(cp.participation_percent(), 75);
    }
}
