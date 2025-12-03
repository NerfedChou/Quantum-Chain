//! Attestation entity
//!
//! Reference: SPEC-09-FINALITY.md Section 2.1

use super::{CheckpointId, ValidatorId};
use bitvec::prelude::*;
use serde::{Deserialize, Serialize};

/// BLS signature for attestations
///
/// Reference: SPEC-09-FINALITY.md Section 2.1
/// Uses BLS12-381 for signature aggregation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlsSignature(pub Vec<u8>);

impl BlsSignature {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Create invalid signature for testing
    #[cfg(test)]
    pub fn invalid() -> Self {
        Self(vec![0u8; 96])
    }
}

impl Default for BlsSignature {
    fn default() -> Self {
        Self(Vec::new())
    }
}

/// Validator attestation for a checkpoint
///
/// Reference: SPEC-09-FINALITY.md Section 2.1
/// An attestation is a validator's vote for a source→target checkpoint link
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attestation {
    /// Validator making the attestation
    pub validator_id: ValidatorId,
    /// Source checkpoint (must be justified)
    pub source_checkpoint: CheckpointId,
    /// Target checkpoint (being voted on)
    pub target_checkpoint: CheckpointId,
    /// BLS signature over the attestation data
    pub signature: BlsSignature,
    /// Slot at which attestation was made
    pub slot: u64,
}

impl Attestation {
    /// Create a new attestation
    pub fn new(
        validator_id: ValidatorId,
        source: CheckpointId,
        target: CheckpointId,
        signature: BlsSignature,
        slot: u64,
    ) -> Self {
        Self {
            validator_id,
            source_checkpoint: source,
            target_checkpoint: target,
            signature,
            slot,
        }
    }

    /// Get the signing message for verification
    ///
    /// Reference: SPEC-09-FINALITY.md Zero-Trust verification
    pub fn signing_message(&self) -> Vec<u8> {
        let mut message = Vec::with_capacity(128);
        message.extend_from_slice(&self.source_checkpoint.epoch.to_le_bytes());
        message.extend_from_slice(&self.source_checkpoint.block_hash);
        message.extend_from_slice(&self.target_checkpoint.epoch.to_le_bytes());
        message.extend_from_slice(&self.target_checkpoint.block_hash);
        message.extend_from_slice(&self.slot.to_le_bytes());
        message
    }

    /// Check if this attestation conflicts with another (slashable)
    ///
    /// Slashable conditions:
    /// 1. Double vote: same target epoch, different target block
    /// 2. Surround vote: source/target surrounds another attestation
    pub fn conflicts_with(&self, other: &Attestation) -> bool {
        if self.validator_id != other.validator_id {
            return false;
        }

        // Double vote: same target epoch, different block
        if self.target_checkpoint.epoch == other.target_checkpoint.epoch
            && self.target_checkpoint.block_hash != other.target_checkpoint.block_hash
        {
            return true;
        }

        // Surround vote: one attestation surrounds the other
        let self_source = self.source_checkpoint.epoch;
        let self_target = self.target_checkpoint.epoch;
        let other_source = other.source_checkpoint.epoch;
        let other_target = other.target_checkpoint.epoch;

        // self surrounds other
        if self_source < other_source && self_target > other_target {
            return true;
        }

        // other surrounds self
        if other_source < self_source && other_target > self_target {
            return true;
        }

        false
    }
}

/// Aggregated attestations for a checkpoint
///
/// Reference: SPEC-09-FINALITY.md Section 2.1
/// Aggregates multiple attestations with same source/target
#[derive(Clone, Debug)]
pub struct AggregatedAttestations {
    /// Source checkpoint
    pub source_checkpoint: CheckpointId,
    /// Target checkpoint
    pub target_checkpoint: CheckpointId,
    /// Individual attestations
    pub attestations: Vec<Attestation>,
    /// Participation bitmap (which validators attested)
    pub participation_bitmap: BitVec<u8, Msb0>,
    /// Aggregated BLS signature (if computed)
    pub aggregate_signature: Option<BlsSignature>,
    /// Total attested stake
    pub total_stake: u128,
}

impl AggregatedAttestations {
    /// Create new aggregated attestations
    pub fn new(source: CheckpointId, target: CheckpointId, validator_count: usize) -> Self {
        Self {
            source_checkpoint: source,
            target_checkpoint: target,
            attestations: Vec::new(),
            participation_bitmap: bitvec![u8, Msb0; 0; validator_count],
            aggregate_signature: None,
            total_stake: 0,
        }
    }

    /// Add an attestation with its stake weight
    pub fn add_attestation(
        &mut self,
        attestation: Attestation,
        validator_index: usize,
        stake: u128,
    ) {
        if validator_index < self.participation_bitmap.len() {
            self.participation_bitmap.set(validator_index, true);
        }
        self.attestations.push(attestation);
        self.total_stake = self.total_stake.saturating_add(stake);
    }

    /// Count participating validators
    pub fn participation_count(&self) -> usize {
        self.participation_bitmap.count_ones()
    }

    /// Check if validator already attested
    pub fn has_attested(&self, validator_index: usize) -> bool {
        self.participation_bitmap
            .get(validator_index)
            .map(|b| *b)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::Hash;

    fn test_hash(n: u8) -> Hash {
        let mut hash = [0u8; 32];
        hash[0] = n;
        hash
    }

    fn test_validator(n: u8) -> ValidatorId {
        let mut id = [0u8; 32];
        id[0] = n;
        ValidatorId(id)
    }

    #[test]
    fn test_double_vote_detection() {
        let source = CheckpointId::new(1, test_hash(1));
        let target1 = CheckpointId::new(2, test_hash(2));
        let target2 = CheckpointId::new(2, test_hash(3)); // Same epoch, different hash

        let att1 = Attestation::new(
            test_validator(1),
            source,
            target1,
            BlsSignature::default(),
            64,
        );

        let att2 = Attestation::new(
            test_validator(1),
            source,
            target2,
            BlsSignature::default(),
            65,
        );

        assert!(att1.conflicts_with(&att2));
    }

    #[test]
    fn test_surround_vote_detection() {
        let source1 = CheckpointId::new(1, test_hash(1));
        let target1 = CheckpointId::new(4, test_hash(4));

        let source2 = CheckpointId::new(2, test_hash(2));
        let target2 = CheckpointId::new(3, test_hash(3));

        let att1 = Attestation::new(
            test_validator(1),
            source1,
            target1,
            BlsSignature::default(),
            128,
        );

        let att2 = Attestation::new(
            test_validator(1),
            source2,
            target2,
            BlsSignature::default(),
            96,
        );

        // att1 (1→4) surrounds att2 (2→3)
        assert!(att1.conflicts_with(&att2));
    }

    #[test]
    fn test_aggregated_attestations() {
        let source = CheckpointId::new(1, test_hash(1));
        let target = CheckpointId::new(2, test_hash(2));

        let mut agg = AggregatedAttestations::new(source, target, 100);

        let att = Attestation::new(
            test_validator(5),
            source,
            target,
            BlsSignature::default(),
            64,
        );

        agg.add_attestation(att, 5, 1000);

        assert_eq!(agg.participation_count(), 1);
        assert!(agg.has_attested(5));
        assert!(!agg.has_attested(6));
        assert_eq!(agg.total_stake, 1000);
    }
}
