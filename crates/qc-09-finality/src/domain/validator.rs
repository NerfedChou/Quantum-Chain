//! Validator entity
//!
//! Reference: SPEC-09-FINALITY.md Section 2.1

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use std::collections::HashMap;

/// BLS public key for signature verification (96 bytes for BLS12-381 G2)
pub type BlsPublicKey = [u8; 96];

/// Validator identifier (derived from public key, 32 bytes)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidatorId(pub [u8; 32]);

impl ValidatorId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for ValidatorId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Validator with stake and public key information
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Validator {
    pub id: ValidatorId,
    pub stake: u128,
    pub index: usize,
    pub active: bool,
    /// BLS public key for signature verification
    /// SECURITY: This is the actual cryptographic key used for attestation verification
    #[serde_as(as = "Bytes")]
    pub pubkey: BlsPublicKey,
}

impl Validator {
    pub fn new(id: ValidatorId, stake: u128, index: usize) -> Self {
        // Default pubkey derived from ID (for backward compatibility)
        let mut pubkey = [0u8; 96];
        pubkey[..32].copy_from_slice(&id.0);
        Self {
            id,
            stake,
            index,
            active: true,
            pubkey,
        }
    }

    /// Create validator with explicit public key
    pub fn with_pubkey(id: ValidatorId, stake: u128, index: usize, pubkey: BlsPublicKey) -> Self {
        Self {
            id,
            stake,
            index,
            active: true,
            pubkey,
        }
    }
}

/// Set of validators for an epoch
///
/// Reference: SPEC-09-FINALITY.md Section 3.2
/// Stake information comes from State Management (Subsystem 4)
#[derive(Clone, Debug, Default)]
pub struct ValidatorSet {
    validators: HashMap<ValidatorId, Validator>,
    index_to_id: Vec<ValidatorId>,
    total_stake: u128,
    epoch: u64,
}

impl ValidatorSet {
    /// Create new validator set for epoch
    pub fn new(epoch: u64) -> Self {
        Self {
            validators: HashMap::new(),
            index_to_id: Vec::new(),
            total_stake: 0,
            epoch,
        }
    }

    /// Add a validator to the set
    pub fn add_validator(&mut self, id: ValidatorId, stake: u128) {
        let index = self.index_to_id.len();
        let validator = Validator::new(id, stake, index);
        self.validators.insert(id, validator);
        self.index_to_id.push(id);
        self.total_stake = self.total_stake.saturating_add(stake);
    }

    /// Add a validator with explicit public key
    pub fn add_validator_with_pubkey(&mut self, id: ValidatorId, stake: u128, pubkey: BlsPublicKey) {
        let index = self.index_to_id.len();
        let validator = Validator::with_pubkey(id, stake, index, pubkey);
        self.validators.insert(id, validator);
        self.index_to_id.push(id);
        self.total_stake = self.total_stake.saturating_add(stake);
    }

    /// Get validator by ID
    pub fn get(&self, id: &ValidatorId) -> Option<&Validator> {
        self.validators.get(id)
    }

    /// Get validator index
    pub fn get_index(&self, id: &ValidatorId) -> Option<usize> {
        self.validators.get(id).map(|v| v.index)
    }

    /// Get validator stake
    pub fn get_stake(&self, id: &ValidatorId) -> Option<u128> {
        self.validators.get(id).map(|v| v.stake)
    }

    /// Get validator public key
    pub fn get_pubkey(&self, id: &ValidatorId) -> Option<&BlsPublicKey> {
        self.validators.get(id).map(|v| &v.pubkey)
    }

    /// Check if validator is in set
    pub fn contains(&self, id: &ValidatorId) -> bool {
        self.validators.contains_key(id)
    }

    /// Get total active stake
    pub fn total_stake(&self) -> u128 {
        self.total_stake
    }

    /// Get validator count
    pub fn len(&self) -> usize {
        self.validators.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.validators.is_empty()
    }

    /// Get epoch
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Iterate over validators
    pub fn iter(&self) -> impl Iterator<Item = &Validator> {
        self.validators.values()
    }

    /// Calculate required stake for justification (2/3 + 1)
    ///
    /// INVARIANT-2: 2/3 threshold
    /// SECURITY: Uses checked arithmetic to prevent overflow
    pub fn required_stake(&self) -> u128 {
        self.total_stake
            .checked_mul(2)
            .map(|v| v / 3 + 1)
            .unwrap_or_else(|| (self.total_stake / 3).saturating_mul(2).saturating_add(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_validator_id(n: u8) -> ValidatorId {
        let mut id = [0u8; 32];
        id[0] = n;
        ValidatorId(id)
    }

    #[test]
    fn test_validator_set_basic() {
        let mut set = ValidatorSet::new(1);

        set.add_validator(test_validator_id(1), 100);
        set.add_validator(test_validator_id(2), 200);
        set.add_validator(test_validator_id(3), 300);

        assert_eq!(set.len(), 3);
        assert_eq!(set.total_stake(), 600);
        assert!(set.contains(&test_validator_id(1)));
        assert!(!set.contains(&test_validator_id(4)));
    }

    #[test]
    fn test_validator_set_stake_lookup() {
        let mut set = ValidatorSet::new(1);
        set.add_validator(test_validator_id(1), 100);

        assert_eq!(set.get_stake(&test_validator_id(1)), Some(100));
        assert_eq!(set.get_stake(&test_validator_id(2)), None);
    }

    #[test]
    fn test_required_stake_calculation() {
        let mut set = ValidatorSet::new(1);

        // 100 validators with 100 stake each = 10000 total
        for i in 0..100 {
            set.add_validator(test_validator_id(i), 100);
        }

        // 2/3 + 1 = 6667
        assert_eq!(set.required_stake(), 6667);
    }

    #[test]
    fn test_validator_with_pubkey() {
        let mut set = ValidatorSet::new(1);
        let id = test_validator_id(1);
        let pubkey = [42u8; 96];
        
        set.add_validator_with_pubkey(id, 100, pubkey);
        
        let retrieved_pubkey = set.get_pubkey(&id);
        assert!(retrieved_pubkey.is_some());
        assert_eq!(retrieved_pubkey.unwrap()[0], 42);
    }
}
