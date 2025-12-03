//! Finality proof entity
//!
//! Reference: SPEC-09-FINALITY.md Section 4.1

use super::{attestation::BlsSignature, Checkpoint};
use bitvec::prelude::*;
use serde::{Deserialize, Serialize};

/// Proof of block finalization
///
/// Reference: SPEC-09-FINALITY.md Section 4.1
/// Contains all data needed to verify a block is finalized
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityProof {
    /// Source checkpoint (previous justified)
    pub source_checkpoint: ProofCheckpoint,
    /// Target checkpoint (newly finalized)
    pub target_checkpoint: ProofCheckpoint,
    /// Aggregated BLS signature
    pub aggregate_signature: BlsSignature,
    /// Participation bitmap showing which validators signed
    pub participation_bitmap: Vec<u8>,
    /// Total participating stake
    pub participating_stake: u128,
    /// Total stake at epoch
    pub total_stake: u128,
}

/// Checkpoint data for proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofCheckpoint {
    pub epoch: u64,
    pub block_hash: [u8; 32],
    pub block_height: u64,
}

impl From<&Checkpoint> for ProofCheckpoint {
    fn from(cp: &Checkpoint) -> Self {
        Self {
            epoch: cp.epoch,
            block_hash: cp.block_hash,
            block_height: cp.block_height,
        }
    }
}

impl FinalityProof {
    /// Create a new finality proof
    pub fn new(
        source: &Checkpoint,
        target: &Checkpoint,
        aggregate_signature: BlsSignature,
        participation_bitmap: BitVec<u8, Msb0>,
        participating_stake: u128,
        total_stake: u128,
    ) -> Self {
        Self {
            source_checkpoint: ProofCheckpoint::from(source),
            target_checkpoint: ProofCheckpoint::from(target),
            aggregate_signature,
            participation_bitmap: participation_bitmap.into_vec(),
            participating_stake,
            total_stake,
        }
    }

    /// Check if proof meets 2/3 threshold
    pub fn is_valid_threshold(&self) -> bool {
        if self.total_stake == 0 {
            return false;
        }
        let required = (self.total_stake * 2) / 3 + 1;
        self.participating_stake >= required
    }

    /// Get participation percentage
    pub fn participation_percent(&self) -> u8 {
        if self.total_stake == 0 {
            return 0;
        }
        ((self.participating_stake * 100) / self.total_stake) as u8
    }

    /// Get participant count from bitmap
    pub fn participant_count(&self) -> usize {
        self.participation_bitmap
            .iter()
            .map(|b| b.count_ones() as usize)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::Hash;

    fn test_checkpoint(epoch: u64) -> Checkpoint {
        let mut hash = [0u8; 32];
        hash[0] = epoch as u8;
        Checkpoint::new(epoch, hash, epoch * 32)
    }

    #[test]
    fn test_proof_threshold_check() {
        let source = test_checkpoint(1);
        let target = test_checkpoint(2);

        let proof = FinalityProof::new(
            &source,
            &target,
            BlsSignature::default(),
            BitVec::new(),
            6700, // 67% participation
            10000,
        );

        assert!(proof.is_valid_threshold());
    }

    #[test]
    fn test_proof_below_threshold() {
        let source = test_checkpoint(1);
        let target = test_checkpoint(2);

        let proof = FinalityProof::new(
            &source,
            &target,
            BlsSignature::default(),
            BitVec::new(),
            6600, // 66% - below threshold
            10000,
        );

        assert!(!proof.is_valid_threshold());
    }

    #[test]
    fn test_participation_percent() {
        let source = test_checkpoint(1);
        let target = test_checkpoint(2);

        let proof = FinalityProof::new(
            &source,
            &target,
            BlsSignature::default(),
            BitVec::new(),
            7500,
            10000,
        );

        assert_eq!(proof.participation_percent(), 75);
    }
}
