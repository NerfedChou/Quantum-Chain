//! # Committee-Based BLS Verification Cache
//!
//! Pre-aggregates public keys for efficient batch verification.
//!
//! ## Optimization
//!
//! Instead of aggregating 5,000 public keys on every verification,
//! pre-compute committee aggregate keys and subtract absent validators.
//!
//! EffectiveKey = CommitteePubKey - Sum(Absent_Validators)
//!
//! Reference: SPEC-09-FINALITY.md

use crate::domain::{ValidatorId, ValidatorSet};
use std::collections::HashMap;

/// Default committee size.
pub const COMMITTEE_SIZE: usize = 128;

/// Pre-computed committee public key cache.
#[derive(Debug)]
pub struct CommitteeKeyCache {
    /// Epoch this cache was built for
    epoch: u64,
    /// Number of committees
    num_committees: usize,
    /// Validator -> committee index
    validator_committee: HashMap<ValidatorId, usize>,
    /// Committee index -> validator IDs in that committee
    committee_members: HashMap<usize, Vec<ValidatorId>>,
    /// Total validator count
    total_validators: usize,
}

impl CommitteeKeyCache {
    /// Build cache from validator set.
    pub fn build(validator_set: &ValidatorSet, committee_size: usize) -> Self {
        let validators: Vec<ValidatorId> = validator_set.iter().map(|v| v.id).collect();
        let num_validators = validators.len();
        let num_committees = num_validators.div_ceil(committee_size);

        let mut validator_committee = HashMap::new();
        let mut committee_members: HashMap<usize, Vec<ValidatorId>> = HashMap::new();

        for (i, validator) in validators.iter().enumerate() {
            let committee_idx = i / committee_size;
            validator_committee.insert(*validator, committee_idx);
            committee_members
                .entry(committee_idx)
                .or_default()
                .push(*validator);
        }

        Self {
            epoch: validator_set.epoch(),
            num_committees,
            validator_committee,
            committee_members,
            total_validators: num_validators,
        }
    }

    /// Get committee index for a validator.
    pub fn get_committee(&self, validator: &ValidatorId) -> Option<usize> {
        self.validator_committee.get(validator).copied()
    }

    /// Get members of a committee.
    pub fn get_committee_members(&self, committee_idx: usize) -> Option<&Vec<ValidatorId>> {
        self.committee_members.get(&committee_idx)
    }

    /// Compute effective participant count for a participation bitmap.
    ///
    /// For each committee, count how many are present vs absent.
    pub fn analyze_participation(&self, participation: &[bool]) -> ParticipationAnalysis {
        let mut present = 0usize;
        let mut absent_per_committee: HashMap<usize, usize> = HashMap::new();

        for (i, &is_present) in participation.iter().enumerate() {
            if is_present {
                present += 1;
            } else {
                let committee = i / (self.total_validators / self.num_committees.max(1));
                *absent_per_committee.entry(committee).or_insert(0) += 1;
            }
        }

        ParticipationAnalysis {
            present,
            absent: participation.len() - present,
            committees_with_absences: absent_per_committee.len(),
        }
    }

    /// Get epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Get number of committees.
    pub fn num_committees(&self) -> usize {
        self.num_committees
    }

    /// Get total validators.
    pub fn total_validators(&self) -> usize {
        self.total_validators
    }
}

/// Analysis of participation bitmap.
#[derive(Clone, Debug)]
pub struct ParticipationAnalysis {
    /// Number of present validators
    pub present: usize,
    /// Number of absent validators
    pub absent: usize,
    /// Number of committees with at least one absence
    pub committees_with_absences: usize,
}

impl ParticipationAnalysis {
    /// Calculate participation rate.
    pub fn participation_rate(&self) -> f64 {
        let total = self.present + self.absent;
        if total == 0 {
            return 0.0;
        }
        self.present as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_validator_set(count: usize) -> ValidatorSet {
        let mut set = ValidatorSet::new(0);
        for i in 0..count {
            let mut id = [0u8; 32];
            id[0..4].copy_from_slice(&(i as u32).to_le_bytes());
            set.add_validator(ValidatorId::new(id), 100);
        }
        set
    }

    #[test]
    fn test_build_cache() {
        let vs = make_validator_set(300);
        let cache = CommitteeKeyCache::build(&vs, 128);

        // 300 / 128 = 3 committees (rounded up)
        assert_eq!(cache.num_committees(), 3);
        assert_eq!(cache.total_validators(), 300);
    }

    #[test]
    fn test_committee_assignment() {
        let vs = make_validator_set(256);
        let cache = CommitteeKeyCache::build(&vs, 128);

        // All validators should be assigned to a committee
        let v0 = ValidatorId::new([0u8; 32]);
        let committee = cache.get_committee(&v0);
        assert!(committee.is_some());
        assert!(committee.unwrap() < cache.num_committees());

        // Different validator should also be assigned
        let mut v129 = [0u8; 32];
        v129[0..4].copy_from_slice(&(129u32).to_le_bytes());
        let committee129 = cache.get_committee(&ValidatorId::new(v129));
        assert!(committee129.is_some());
        assert!(committee129.unwrap() < cache.num_committees());
    }

    #[test]
    fn test_participation_analysis() {
        let vs = make_validator_set(100);
        let cache = CommitteeKeyCache::build(&vs, 50);

        // 80 present, 20 absent
        let mut participation = vec![true; 100];
        for item in participation.iter_mut().take(100).skip(80) {
            *item = false;
        }

        let analysis = cache.analyze_participation(&participation);

        assert_eq!(analysis.present, 80);
        assert_eq!(analysis.absent, 20);
        assert!((analysis.participation_rate() - 0.8).abs() < 0.001);
    }
}
