//! # Inclusion Delay Tracking
//!
//! Rewards timely attestations and penalizes late ones.
//!
//! ## Economic Incentive Design
//!
//! Validators are incentivized to include attestations quickly:
//! - Immediate inclusion (slot+1): 100% reward
//! - Delayed inclusion: Decaying reward based on slots elapsed
//!
//! Reference: SPEC-09-FINALITY.md Phase 2

use shared_types::Hash;

/// Maximum slots for attestation inclusion.
pub const MAX_INCLUSION_DELAY: u64 = 32;

/// Reward curve type.
#[derive(Clone, Debug)]
#[derive(Default)]
pub enum RewardCurve {
    /// Linear decay: reward = base * (1 - delay/max)
    #[default]
    Linear,
    /// Exponential decay: reward = base * 0.5^(delay/halflife)
    Exponential { halflife: u64 },
    /// Step function: full until threshold, then zero
    Step { threshold: u64 },
}


/// Inclusion delay tracker for attestation rewards.
#[derive(Clone, Debug)]
pub struct InclusionDelayTracker {
    /// Maximum slots for attestation inclusion
    pub max_inclusion_delay: u64,
    /// Reward calculation curve
    pub reward_curve: RewardCurve,
}

impl Default for InclusionDelayTracker {
    fn default() -> Self {
        Self {
            max_inclusion_delay: MAX_INCLUSION_DELAY,
            reward_curve: RewardCurve::Linear,
        }
    }
}

impl InclusionDelayTracker {
    pub fn new(max_delay: u64, curve: RewardCurve) -> Self {
        Self {
            max_inclusion_delay: max_delay,
            reward_curve: curve,
        }
    }

    /// Calculate inclusion reward based on delay.
    ///
    /// Earlier inclusion = higher reward.
    pub fn calculate_reward(&self, delay_slots: u64, base_reward: u128) -> u128 {
        if delay_slots == 0 {
            return base_reward;
        }

        if delay_slots > self.max_inclusion_delay {
            return 0;
        }

        match &self.reward_curve {
            RewardCurve::Linear => {
                // reward = base * (1 - delay/max)
                let remaining = self.max_inclusion_delay - delay_slots;
                (base_reward * remaining as u128) / self.max_inclusion_delay as u128
            }
            RewardCurve::Exponential { halflife } => {
                // reward = base * 0.5^(delay/halflife)
                let halvings = delay_slots / halflife;
                if halvings >= 64 {
                    return 0;
                }
                base_reward >> halvings
            }
            RewardCurve::Step { threshold } => {
                if delay_slots <= *threshold {
                    base_reward
                } else {
                    0
                }
            }
        }
    }

    /// Check if attestation is still valid for inclusion.
    pub fn is_valid_for_inclusion(&self, attestation_slot: u64, current_slot: u64) -> bool {
        if current_slot < attestation_slot {
            return false; // Future attestation
        }

        let delay = current_slot - attestation_slot;
        delay <= self.max_inclusion_delay
    }

    /// Calculate penalty for late inclusion.
    ///
    /// Used when attestation is included but very late.
    pub fn calculate_penalty(&self, delay_slots: u64, base_penalty: u128) -> u128 {
        if delay_slots <= self.max_inclusion_delay / 2 {
            return 0; // No penalty for reasonable delays
        }

        let excess_delay = delay_slots - (self.max_inclusion_delay / 2);
        let max_excess = self.max_inclusion_delay / 2;

        if excess_delay >= max_excess {
            base_penalty
        } else {
            (base_penalty * excess_delay as u128) / max_excess as u128
        }
    }
}

/// Attestation inclusion record.
#[derive(Clone, Debug)]
pub struct InclusionRecord {
    /// Attestation hash
    pub attestation_hash: Hash,
    /// Slot when attestation was created
    pub attestation_slot: u64,
    /// Slot when attestation was included
    pub inclusion_slot: u64,
    /// Validator who created the attestation
    pub validator_id: [u8; 32],
    /// Calculated reward
    pub reward: u128,
}

impl InclusionRecord {
    pub fn delay(&self) -> u64 {
        self.inclusion_slot.saturating_sub(self.attestation_slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_reward_immediate() {
        let tracker = InclusionDelayTracker::default();

        let reward = tracker.calculate_reward(0, 1000);
        assert_eq!(reward, 1000);
    }

    #[test]
    fn test_linear_reward_half_delay() {
        let tracker = InclusionDelayTracker::default();

        // 16 slots delay out of 32 max = 50% reward
        let reward = tracker.calculate_reward(16, 1000);
        assert_eq!(reward, 500);
    }

    #[test]
    fn test_linear_reward_expired() {
        let tracker = InclusionDelayTracker::default();

        let reward = tracker.calculate_reward(33, 1000);
        assert_eq!(reward, 0);
    }

    #[test]
    fn test_exponential_reward() {
        let tracker = InclusionDelayTracker::new(32, RewardCurve::Exponential { halflife: 8 });

        // 0 delay = full
        assert_eq!(tracker.calculate_reward(0, 1000), 1000);

        // 8 slots = half
        assert_eq!(tracker.calculate_reward(8, 1000), 500);

        // 16 slots = quarter
        assert_eq!(tracker.calculate_reward(16, 1000), 250);
    }

    #[test]
    fn test_step_reward() {
        let tracker = InclusionDelayTracker::new(32, RewardCurve::Step { threshold: 4 });

        assert_eq!(tracker.calculate_reward(3, 1000), 1000);
        assert_eq!(tracker.calculate_reward(4, 1000), 1000);
        assert_eq!(tracker.calculate_reward(5, 1000), 0);
    }

    #[test]
    fn test_validity_check() {
        let tracker = InclusionDelayTracker::default();

        // Valid: current=100, attestation=90 (10 slot delay)
        assert!(tracker.is_valid_for_inclusion(90, 100));

        // Invalid: current=100, attestation=50 (50 slot delay)
        assert!(!tracker.is_valid_for_inclusion(50, 100));

        // Invalid: future attestation
        assert!(!tracker.is_valid_for_inclusion(110, 100));
    }
}
