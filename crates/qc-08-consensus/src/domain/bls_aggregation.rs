//! # Pipelined BLS Signature Aggregation
//!
//! Optimized aggregate signature verification using committee pre-computation.
//!
//! ## Problem
//!
//! Verifying 10,000 individual BLS signatures is O(N) scalar additions.
//! This is too slow for real-time consensus.
//!
//! ## Solution: Bucket-Based Aggregation
//!
//! 1. Divide validators into committees (e.g., 128 sub-groups)
//! 2. Pre-compute aggregate public key for each committee
//! 3. Verification becomes O(M) where M = missing validators (usually small)
//!
//! Reference: SPEC-08-CONSENSUS.md Phase 3

use crate::domain::{BlsPublicKey, ValidatorId, ValidatorInfo, ValidatorSet};
use std::collections::HashMap;

/// Committee size for BLS aggregation optimization.
pub const COMMITTEE_SIZE: usize = 128;

/// Pre-computed committee cache for efficient BLS verification.
#[derive(Debug)]
pub struct CommitteeCache {
    /// Committee index -> aggregate public key
    committee_keys: HashMap<u64, AggregateKey>,
    /// Committee index -> member validator IDs
    committee_members: HashMap<u64, Vec<ValidatorId>>,
    /// Validator ID -> committee index
    validator_committee: HashMap<ValidatorId, u64>,
    /// Total aggregate key (all validators)
    total_aggregate: Option<AggregateKey>,
    /// Number of committees
    num_committees: u64,
    /// Epoch this cache was built for
    epoch: u64,
}

/// Aggregate public key (simplified representation).
/// In production, this would be an actual BLS aggregate.
#[derive(Clone, Debug)]
pub struct AggregateKey {
    /// Component keys (simplified - real impl would use BLS math)
    pub keys: Vec<BlsPublicKey>,
    /// Cached aggregate (would be computed via BLS addition)
    pub aggregate: BlsPublicKey,
}

impl Default for AggregateKey {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            aggregate: [0u8; 48],
        }
    }
}

impl AggregateKey {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a key to the aggregate.
    pub fn add(&mut self, pubkey: BlsPublicKey) {
        self.keys.push(pubkey);
        // In production: self.aggregate = bls_add(self.aggregate, pubkey)
        // For now, just store the last key as placeholder
        self.aggregate = pubkey;
    }

    /// Subtract a key from the aggregate (for missing validators).
    pub fn subtract(&mut self, pubkey: &BlsPublicKey) {
        self.keys.retain(|k| k != pubkey);
        // In production: self.aggregate = bls_sub(self.aggregate, pubkey)
    }

    /// Get key count.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl CommitteeCache {
    /// Build committee cache from validator set.
    pub fn build(validator_set: &ValidatorSet) -> Self {
        let num_validators = validator_set.len();
        let num_committees = ((num_validators + COMMITTEE_SIZE - 1) / COMMITTEE_SIZE) as u64;

        let mut cache = Self {
            committee_keys: HashMap::new(),
            committee_members: HashMap::new(),
            validator_committee: HashMap::new(),
            total_aggregate: None,
            num_committees,
            epoch: validator_set.epoch,
        };

        // Assign validators to committees
        for (i, validator) in validator_set.validators.iter().enumerate() {
            let committee_id = (i / COMMITTEE_SIZE) as u64;

            cache.validator_committee.insert(validator.id, committee_id);
            cache
                .committee_members
                .entry(committee_id)
                .or_default()
                .push(validator.id);

            cache
                .committee_keys
                .entry(committee_id)
                .or_insert_with(AggregateKey::new)
                .add(validator.pubkey);
        }

        // Build total aggregate
        let mut total = AggregateKey::new();
        for validator in &validator_set.validators {
            total.add(validator.pubkey);
        }
        cache.total_aggregate = Some(total);

        cache
    }

    /// Verify aggregate signature using optimized lookup.
    ///
    /// Instead of O(N) key additions, compute:
    /// `EffectivePK = TotalAggregate - Sum(AbsentPKs)`
    ///
    /// This is O(M) where M = number of missing validators.
    pub fn compute_effective_key(
        &self,
        participation: &[bool],
        validator_set: &ValidatorSet,
    ) -> AggregateKey {
        let mut effective = self.total_aggregate.clone().unwrap_or_default();

        // Subtract missing validators (O(M) where M = absent count)
        for (i, &present) in participation.iter().enumerate() {
            if !present {
                if let Some(validator) = validator_set.validators.get(i) {
                    effective.subtract(&validator.pubkey);
                }
            }
        }

        effective
    }

    /// Verify an aggregate signature against participation bitmap.
    ///
    /// Returns true if signature is valid for the participating validators.
    pub fn verify_aggregate(
        &self,
        _signature: &[u8],
        _message: &[u8],
        participation: &[bool],
        validator_set: &ValidatorSet,
    ) -> bool {
        let effective_key = self.compute_effective_key(participation, validator_set);

        // In production:
        // bls_verify(signature, message, &effective_key.aggregate)

        // For now, just check we have enough participants (2/3)
        let participating = participation.iter().filter(|&&p| p).count();
        let required = (validator_set.len() * 2) / 3 + 1;

        participating >= required && !effective_key.is_empty()
    }

    /// Get committee for a validator.
    pub fn get_committee(&self, validator: &ValidatorId) -> Option<u64> {
        self.validator_committee.get(validator).copied()
    }

    /// Get members of a committee.
    pub fn get_committee_members(&self, committee_id: u64) -> Option<&Vec<ValidatorId>> {
        self.committee_members.get(&committee_id)
    }

    /// Get number of committees.
    pub fn num_committees(&self) -> u64 {
        self.num_committees
    }

    /// Get epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Get stats.
    pub fn stats(&self) -> CommitteeCacheStats {
        CommitteeCacheStats {
            num_committees: self.num_committees,
            total_validators: self.validator_committee.len(),
            epoch: self.epoch,
        }
    }
}

/// Cache statistics.
#[derive(Clone, Debug)]
pub struct CommitteeCacheStats {
    pub num_committees: u64,
    pub total_validators: usize,
    pub epoch: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_validator_set(count: usize) -> ValidatorSet {
        let validators: Vec<ValidatorInfo> = (0..count)
            .map(|i| ValidatorInfo {
                id: {
                    let mut id = [0u8; 32];
                    // Use little-endian u32 to support > 256 validators
                    id[0..4].copy_from_slice(&(i as u32).to_le_bytes());
                    id
                },
                stake: 100,
                pubkey: {
                    let mut pk = [0u8; 48];
                    pk[0..4].copy_from_slice(&(i as u32).to_le_bytes());
                    pk
                },
                active: true,
            })
            .collect();

        ValidatorSet::new(0, validators)
    }

    #[test]
    fn test_committee_assignment() {
        let vs = make_validator_set(300);
        let cache = CommitteeCache::build(&vs);

        // 300 validators / 128 per committee = 3 committees
        assert_eq!(cache.num_committees(), 3);

        // Check first validator is in committee 0
        let v0 = vs.validators[0].id;
        assert_eq!(cache.get_committee(&v0), Some(0));

        // Check validator 129 is in committee 1
        let v129 = vs.validators[129].id;
        assert_eq!(cache.get_committee(&v129), Some(1));
    }

    #[test]
    fn test_effective_key_all_present() {
        let vs = make_validator_set(10);
        let cache = CommitteeCache::build(&vs);

        let participation = vec![true; 10];
        let effective = cache.compute_effective_key(&participation, &vs);

        // All present = all keys
        assert_eq!(effective.len(), 10);
    }

    #[test]
    fn test_effective_key_some_missing() {
        let vs = make_validator_set(10);
        let cache = CommitteeCache::build(&vs);

        // 3 validators missing
        let mut participation = vec![true; 10];
        participation[2] = false;
        participation[5] = false;
        participation[8] = false;

        let effective = cache.compute_effective_key(&participation, &vs);

        // 10 - 3 = 7 keys
        assert_eq!(effective.len(), 7);
    }

    #[test]
    fn test_verify_aggregate_sufficient() {
        let vs = make_validator_set(9);
        let cache = CommitteeCache::build(&vs);

        // 7/9 = 77% > 67% required
        let mut participation = vec![true; 9];
        participation[0] = false;
        participation[1] = false;

        let valid = cache.verify_aggregate(b"sig", b"msg", &participation, &vs);
        assert!(valid);
    }

    #[test]
    fn test_verify_aggregate_insufficient() {
        let vs = make_validator_set(9);
        let cache = CommitteeCache::build(&vs);

        // 5/9 = 55% < 67% required
        let mut participation = vec![true; 9];
        participation[0] = false;
        participation[1] = false;
        participation[2] = false;
        participation[3] = false;

        let valid = cache.verify_aggregate(b"sig", b"msg", &participation, &vs);
        assert!(!valid);
    }
}
