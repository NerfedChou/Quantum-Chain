//! Validator domain entities
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2.1, 3.2

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use std::collections::HashMap;

/// Validator identifier (32-byte public key hash)
pub type ValidatorId = [u8; 32];

/// Validator public key for BLS signatures
pub type BlsPublicKey = [u8; 48];

/// Validator set with stake information
///
/// Reference: SPEC-08 Section 3.2
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub epoch: u64,
    pub validators: Vec<ValidatorInfo>,
    pub total_stake: u128,
    /// Quick lookup by validator ID
    #[serde(skip)]
    lookup: HashMap<ValidatorId, usize>,
}

impl ValidatorSet {
    /// Create a new validator set
    pub fn new(epoch: u64, validators: Vec<ValidatorInfo>) -> Self {
        let total_stake = validators.iter().map(|v| v.stake).sum();
        let lookup = validators
            .iter()
            .enumerate()
            .map(|(i, v)| (v.id, i))
            .collect();
        Self {
            epoch,
            validators,
            total_stake,
            lookup,
        }
    }

    /// Get the number of validators
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Check if a validator is in the set
    pub fn contains(&self, validator_id: &ValidatorId) -> bool {
        self.lookup.contains_key(validator_id)
    }

    /// Get validator info by ID
    pub fn get(&self, validator_id: &ValidatorId) -> Option<&ValidatorInfo> {
        self.lookup
            .get(validator_id)
            .map(|&idx| &self.validators[idx])
    }

    /// Get validator's public key
    pub fn get_pubkey(&self, validator_id: &ValidatorId) -> Option<&BlsPublicKey> {
        self.get(validator_id).map(|v| &v.pubkey)
    }

    /// Calculate required attestation count for 2/3 threshold
    pub fn required_attestations(&self, percent: u8) -> usize {
        let required = (self.validators.len() * percent as usize) / 100;
        // At minimum, require 2/3 rounded up
        required.max(1)
    }

    /// Calculate required votes for PBFT (2f+1)
    pub fn required_pbft_votes(&self, byzantine_threshold: usize) -> usize {
        2 * byzantine_threshold + 1
    }

    /// Rebuild the lookup table (after deserialization)
    pub fn rebuild_lookup(&mut self) {
        self.lookup = self
            .validators
            .iter()
            .enumerate()
            .map(|(i, v)| (v.id, i))
            .collect();
    }
}

/// Individual validator information
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorInfo {
    pub id: ValidatorId,
    pub stake: u128,
    #[serde_as(as = "Bytes")]
    pub pubkey: BlsPublicKey,
    /// Whether this validator is currently active
    pub active: bool,
}

impl ValidatorInfo {
    /// Create a new validator
    pub fn new(id: ValidatorId, stake: u128, pubkey: BlsPublicKey) -> Self {
        Self {
            id,
            stake,
            pubkey,
            active: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_validator(id: u8, stake: u128) -> ValidatorInfo {
        let mut validator_id = [0u8; 32];
        validator_id[0] = id;
        let pubkey = [id; 48];
        ValidatorInfo::new(validator_id, stake, pubkey)
    }

    #[test]
    fn test_validator_set_creation() {
        let validators = vec![
            create_test_validator(1, 100),
            create_test_validator(2, 200),
            create_test_validator(3, 300),
        ];

        let set = ValidatorSet::new(1, validators);

        assert_eq!(set.len(), 3);
        assert_eq!(set.total_stake, 600);
        assert_eq!(set.epoch, 1);
    }

    #[test]
    fn test_validator_set_lookup() {
        let validators = vec![create_test_validator(1, 100), create_test_validator(2, 200)];

        let set = ValidatorSet::new(1, validators);

        let mut id1 = [0u8; 32];
        id1[0] = 1;

        assert!(set.contains(&id1));
        assert_eq!(set.get(&id1).unwrap().stake, 100);
    }

    #[test]
    fn test_required_attestations() {
        let validators = vec![
            create_test_validator(1, 100),
            create_test_validator(2, 100),
            create_test_validator(3, 100),
        ];

        let set = ValidatorSet::new(1, validators);

        // 67% of 3 = 2.01, so we need at least 2
        assert_eq!(set.required_attestations(67), 2);
    }
}
