//! # Quadratic Inactivity Leak
//!
//! Restores chain liveness when finality stalls due to offline validators.
//!
//! ## Problem
//!
//! Casper FFG requires >2/3 stake to finalize. If 40% of validators go offline,
//! the chain stops finalizing forever without intervention.
//!
//! ## Solution: Quadratic-Stake-Drain
//!
//! 1. Trigger: FinalityLag > 4 epochs
//! 2. Mechanism: Penalty = BasePenalty + (InactivityScore^2)
//! 3. Result: Active validators become the new 67% majority
//! 4. Recovery: Once finality resumes, the leak stops
//!
//! Reference: SPEC-09-FINALITY.md, Ethereum Casper

use crate::domain::ValidatorId;
use std::collections::HashMap;

/// Default epochs before inactivity leak starts.
pub const INACTIVITY_LEAK_EPOCHS: u64 = 4;

/// Base penalty rate in basis points (1 bp = 0.01%).
pub const BASE_PENALTY_BPS: u64 = 100; // 1% per epoch

/// Inactivity leak configuration.
#[derive(Clone, Debug)]
pub struct InactivityLeakConfig {
    /// Epochs without finality before leak starts
    pub leak_threshold_epochs: u64,
    /// Base penalty in basis points per epoch
    pub base_penalty_bps: u64,
    /// Quadratic multiplier
    pub quadratic_factor: u64,
}

impl Default for InactivityLeakConfig {
    fn default() -> Self {
        Self {
            leak_threshold_epochs: INACTIVITY_LEAK_EPOCHS,
            base_penalty_bps: BASE_PENALTY_BPS,
            quadratic_factor: 1,
        }
    }
}

/// Inactivity score for a validator.
#[derive(Clone, Debug, Default)]
pub struct InactivityScore {
    /// Consecutive epochs validator was inactive during leak
    pub epochs_inactive: u64,
    /// Total penalty applied so far
    pub total_penalty: u128,
}

/// Inactivity leak tracker.
#[derive(Debug, Default)]
pub struct InactivityLeakTracker {
    /// Configuration
    config: InactivityLeakConfig,
    /// Inactivity scores per validator
    scores: HashMap<ValidatorId, InactivityScore>,
    /// Current finality lag in epochs
    finality_lag: u64,
    /// Is leak currently active
    leak_active: bool,
    /// Epoch when leak started
    leak_start_epoch: Option<u64>,
}

impl InactivityLeakTracker {
    pub fn new(config: InactivityLeakConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    /// Update finality lag and check if leak should activate.
    pub fn update_finality_lag(&mut self, epochs_since_finality: u64, current_epoch: u64) {
        self.finality_lag = epochs_since_finality;
        
        if epochs_since_finality > self.config.leak_threshold_epochs {
            if !self.leak_active {
                self.leak_active = true;
                self.leak_start_epoch = Some(current_epoch);
            }
        } else {
            // Finality restored - stop leak
            self.leak_active = false;
            self.leak_start_epoch = None;
            // Reset all scores
            self.scores.clear();
        }
    }

    /// Check if inactivity leak is currently active.
    pub fn is_leak_active(&self) -> bool {
        self.leak_active
    }

    /// Record validator participation (or lack thereof) for an epoch.
    ///
    /// Call this for each validator after processing attestations.
    pub fn record_participation(&mut self, validator: ValidatorId, participated: bool) {
        if !self.leak_active {
            return;
        }

        let score = self.scores.entry(validator).or_default();
        
        if participated {
            // Reset inactivity count on participation
            score.epochs_inactive = 0;
        } else {
            // Increment inactivity count
            score.epochs_inactive += 1;
        }
    }

    /// Calculate penalty for a validator using quadratic formula.
    ///
    /// Penalty = BasePenalty + (InactivityScore^2 * QuadraticFactor)
    pub fn calculate_penalty(&self, validator: &ValidatorId, stake: u128) -> u128 {
        if !self.leak_active {
            return 0;
        }

        let score = match self.scores.get(validator) {
            Some(s) => s,
            None => return 0,
        };

        if score.epochs_inactive == 0 {
            return 0;
        }

        // Base penalty (percentage of stake)
        let base = stake * self.config.base_penalty_bps as u128 / 10_000;
        
        // Quadratic component
        let quadratic = score.epochs_inactive.saturating_mul(score.epochs_inactive)
            .saturating_mul(self.config.quadratic_factor) as u128;
        
        base.saturating_add(quadratic)
    }

    /// Apply penalties and return total penalty per validator.
    pub fn apply_penalties(&mut self, stakes: &HashMap<ValidatorId, u128>) -> Vec<(ValidatorId, u128)> {
        if !self.leak_active {
            return Vec::new();
        }

        let mut penalties = Vec::new();
        
        for (validator, stake) in stakes {
            let penalty = self.calculate_penalty(validator, *stake);
            if penalty > 0 {
                if let Some(score) = self.scores.get_mut(validator) {
                    score.total_penalty = score.total_penalty.saturating_add(penalty);
                }
                penalties.push((*validator, penalty));
            }
        }
        
        penalties
    }

    /// Get current finality lag.
    pub fn finality_lag(&self) -> u64 {
        self.finality_lag
    }

    /// Get inactivity score for a validator.
    pub fn get_score(&self, validator: &ValidatorId) -> Option<&InactivityScore> {
        self.scores.get(validator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validator(id: u8) -> ValidatorId {
        ValidatorId::new([id; 32])
    }

    #[test]
    fn test_leak_activates_after_threshold() {
        let mut tracker = InactivityLeakTracker::new(InactivityLeakConfig::default());
        
        tracker.update_finality_lag(3, 10);
        assert!(!tracker.is_leak_active());
        
        tracker.update_finality_lag(5, 12);
        assert!(tracker.is_leak_active());
    }

    #[test]
    fn test_leak_stops_when_finality_resumes() {
        let mut tracker = InactivityLeakTracker::new(InactivityLeakConfig::default());
        
        tracker.update_finality_lag(5, 10);
        assert!(tracker.is_leak_active());
        
        tracker.update_finality_lag(2, 15);
        assert!(!tracker.is_leak_active());
    }

    #[test]
    fn test_quadratic_penalty() {
        let mut tracker = InactivityLeakTracker::new(InactivityLeakConfig::default());
        
        tracker.update_finality_lag(5, 10);
        
        // Record 3 epochs of inactivity
        tracker.record_participation(validator(1), false);
        tracker.record_participation(validator(1), false);
        tracker.record_participation(validator(1), false);
        
        let stake = 1_000_000u128;
        let penalty = tracker.calculate_penalty(&validator(1), stake);
        
        // Base: 1% of 1M = 10,000
        // Quadratic: 3^2 * 1 = 9
        // Total: 10,009
        assert_eq!(penalty, 10_009);
    }

    #[test]
    fn test_participation_resets_score() {
        let mut tracker = InactivityLeakTracker::new(InactivityLeakConfig::default());
        
        tracker.update_finality_lag(5, 10);
        
        tracker.record_participation(validator(1), false);
        tracker.record_participation(validator(1), false);
        
        assert_eq!(tracker.get_score(&validator(1)).unwrap().epochs_inactive, 2);
        
        tracker.record_participation(validator(1), true);
        
        assert_eq!(tracker.get_score(&validator(1)).unwrap().epochs_inactive, 0);
    }
}
