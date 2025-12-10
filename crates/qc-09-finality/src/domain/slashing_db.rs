//! # Slashing Database (Casper Rules Enforcer)
//!
//! Enforces Casper commandments to prevent safety violations.
//!
//! ## The Two Commandments
//!
//! 1. No Double Vote: Cannot vote for two blocks at the same target epoch
//! 2. No Surround Vote: Cannot vote S->T if already voted S'->T' where S<S' and T'>T
//!
//! Reference: SPEC-09-FINALITY.md, Casper FFG Paper

use crate::domain::ValidatorId;
use shared_types::Hash;
use std::collections::HashMap;

/// Slashing evidence for broadcast to consensus.
#[derive(Clone, Debug)]
pub enum SlashingEvidence {
    /// Double vote: two attestations for same target epoch, different blocks
    DoubleVote {
        validator: ValidatorId,
        target_epoch: u64,
        block_a: Hash,
        block_b: Hash,
    },
    /// Surround vote: one attestation surrounds another
    SurroundVote {
        validator: ValidatorId,
        inner_source: u64,
        inner_target: u64,
        outer_source: u64,
        outer_target: u64,
    },
}

impl SlashingEvidence {
    pub fn validator(&self) -> ValidatorId {
        match self {
            Self::DoubleVote { validator, .. } => *validator,
            Self::SurroundVote { validator, .. } => *validator,
        }
    }
}

/// Validator vote record for slashing detection.
#[derive(Clone, Debug, Default)]
pub struct VoteRecord {
    /// Highest source epoch voted for
    pub highest_source: u64,
    /// Highest target epoch voted for
    pub highest_target: u64,
    /// Target epoch -> block hash mapping
    pub target_votes: HashMap<u64, Hash>,
}

/// Slashing database for Casper rule enforcement.
#[derive(Debug, Default)]
pub struct SlashingDb {
    /// Vote records per validator
    records: HashMap<ValidatorId, VoteRecord>,
    /// Detected slashing evidence
    pending_evidence: Vec<SlashingEvidence>,
}

impl SlashingDb {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if an attestation would be slashable and record it.
    ///
    /// Returns `Some(SlashingEvidence)` if the attestation violates Casper rules.
    pub fn check_and_record(
        &mut self,
        validator: ValidatorId,
        source_epoch: u64,
        target_epoch: u64,
        target_block: Hash,
    ) -> Option<SlashingEvidence> {
        let record = self.records.entry(validator).or_default();

        // Commandment 1: No Double Vote
        if let Some(existing_block) = record.target_votes.get(&target_epoch) {
            if *existing_block != target_block {
                let evidence = SlashingEvidence::DoubleVote {
                    validator,
                    target_epoch,
                    block_a: *existing_block,
                    block_b: target_block,
                };
                self.pending_evidence.push(evidence.clone());
                return Some(evidence);
            }
            // Same vote, idempotent
            return None;
        }

        // Commandment 2: No Surround Vote
        // Check if new vote surrounds any existing vote
        if source_epoch < record.highest_source && target_epoch > record.highest_target {
            let evidence = SlashingEvidence::SurroundVote {
                validator,
                inner_source: record.highest_source,
                inner_target: record.highest_target,
                outer_source: source_epoch,
                outer_target: target_epoch,
            };
            self.pending_evidence.push(evidence.clone());
            return Some(evidence);
        }

        // Check if any existing vote surrounds new vote
        if record.highest_source < source_epoch && record.highest_target > target_epoch {
            let evidence = SlashingEvidence::SurroundVote {
                validator,
                inner_source: source_epoch,
                inner_target: target_epoch,
                outer_source: record.highest_source,
                outer_target: record.highest_target,
            };
            self.pending_evidence.push(evidence.clone());
            return Some(evidence);
        }

        // Valid vote - record it
        record.target_votes.insert(target_epoch, target_block);
        if source_epoch > record.highest_source {
            record.highest_source = source_epoch;
        }
        if target_epoch > record.highest_target {
            record.highest_target = target_epoch;
        }

        None
    }

    /// Get pending slashing evidence.
    pub fn drain_pending_evidence(&mut self) -> Vec<SlashingEvidence> {
        std::mem::take(&mut self.pending_evidence)
    }

    /// Get vote record for a validator.
    pub fn get_record(&self, validator: &ValidatorId) -> Option<&VoteRecord> {
        self.records.get(validator)
    }

    /// Prune old epochs to prevent unbounded growth.
    pub fn prune_before(&mut self, epoch: u64) {
        for record in self.records.values_mut() {
            record.target_votes.retain(|&e, _| e >= epoch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validator(id: u8) -> ValidatorId {
        ValidatorId::new([id; 32])
    }

    fn block_hash(id: u8) -> Hash {
        [id; 32]
    }

    #[test]
    fn test_valid_vote() {
        let mut db = SlashingDb::new();
        
        let result = db.check_and_record(validator(1), 5, 10, block_hash(0xAB));
        assert!(result.is_none());
        
        let record = db.get_record(&validator(1)).unwrap();
        assert_eq!(record.highest_source, 5);
        assert_eq!(record.highest_target, 10);
    }

    #[test]
    fn test_double_vote_detection() {
        let mut db = SlashingDb::new();
        
        // First vote
        db.check_and_record(validator(1), 5, 10, block_hash(0xAB));
        
        // Second vote at same target epoch, different block
        let result = db.check_and_record(validator(1), 5, 10, block_hash(0xCD));
        
        assert!(matches!(result, Some(SlashingEvidence::DoubleVote { .. })));
    }

    #[test]
    fn test_surround_vote_detection() {
        let mut db = SlashingDb::new();
        
        // First vote: 2 -> 3
        db.check_and_record(validator(1), 2, 3, block_hash(0xAB));
        
        // Second vote: 1 -> 4 (surrounds first)
        let result = db.check_and_record(validator(1), 1, 4, block_hash(0xCD));
        
        assert!(matches!(result, Some(SlashingEvidence::SurroundVote { .. })));
    }

    #[test]
    fn test_same_vote_is_idempotent() {
        let mut db = SlashingDb::new();
        
        db.check_and_record(validator(1), 5, 10, block_hash(0xAB));
        let result = db.check_and_record(validator(1), 5, 10, block_hash(0xAB));
        
        assert!(result.is_none());
    }
}
