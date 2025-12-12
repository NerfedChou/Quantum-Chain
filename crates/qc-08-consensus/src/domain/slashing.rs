//! # Slashing Module - Double-Vote Detection
//!
//! Nothing-at-Stake defense: detects and punishes validator equivocation.
//!
//! ## Threat Model
//!
//! In PoS, it costs nothing for a validator to sign conflicting blocks
//! at the same height. This module detects such violations and generates
//! slashing evidence for network-wide punishment.
//!
//! ## Algorithm: Double-Vote Detection
//!
//! 1. Maintain SlashingDB of (ValidatorId, Epoch) -> BlockHash
//! 2. When attestation arrives, check for conflicting previous vote
//! 3. If found, generate SlashingEvidence and burn 100% of stake
//!
//! Reference: SPEC-08-CONSENSUS.md Phase 1 Security

use crate::domain::ValidatorId;
use shared_types::Hash;
use std::collections::HashMap;

/// Slashing evidence types.
#[derive(Clone, Debug, PartialEq)]
pub enum SlashingEvidence {
    /// Validator signed two different blocks at same epoch
    DoubleVote {
        validator: ValidatorId,
        epoch: u64,
        vote_a: Hash,
        vote_b: Hash,
        signature_a: Option<Vec<u8>>,
        signature_b: Option<Vec<u8>>,
    },
    /// Validator made surrounding vote (Casper FFG violation)
    SurroundVote {
        validator: ValidatorId,
        inner_source: u64,
        inner_target: u64,
        outer_source: u64,
        outer_target: u64,
    },
}

impl SlashingEvidence {
    /// Get the validator being slashed.
    pub fn validator(&self) -> ValidatorId {
        match self {
            Self::DoubleVote { validator, .. } => *validator,
            Self::SurroundVote { validator, .. } => *validator,
        }
    }

    /// Get the epoch of the violation.
    pub fn epoch(&self) -> u64 {
        match self {
            Self::DoubleVote { epoch, .. } => *epoch,
            Self::SurroundVote { inner_target, .. } => *inner_target,
        }
    }
}

/// Slashing database for tracking validator votes.
///
/// Stores votes indexed by (validator, epoch) to detect double-voting.
#[derive(Debug, Default)]
pub struct SlashingDB {
    /// (validator, epoch) -> block_hash
    votes: HashMap<(ValidatorId, u64), Hash>,
    /// Detected slashing evidence
    pending_slashings: Vec<SlashingEvidence>,
    /// Total slashings recorded
    total_slashings: u64,
}

impl SlashingDB {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check attestation and record vote.
    ///
    /// Returns `Some(SlashingEvidence)` if double-vote detected.
    pub fn check_and_record(
        &mut self,
        validator: ValidatorId,
        epoch: u64,
        block_hash: Hash,
        signature: Option<Vec<u8>>,
    ) -> Option<SlashingEvidence> {
        let key = (validator, epoch);

        if let Some(prev_hash) = self.votes.get(&key) {
            if *prev_hash != block_hash {
                // DOUBLE VOTE DETECTED!
                let evidence = SlashingEvidence::DoubleVote {
                    validator,
                    epoch,
                    vote_a: *prev_hash,
                    vote_b: block_hash,
                    signature_a: None, // Could store for proof
                    signature_b: signature,
                };
                self.pending_slashings.push(evidence.clone());
                self.total_slashings += 1;
                return Some(evidence);
            }
            // Same vote, idempotent
            return None;
        }

        // First vote at this epoch
        self.votes.insert(key, block_hash);
        None
    }

    /// Check for surround vote (Casper FFG).
    ///
    /// A surround vote occurs when:
    /// - New vote: source_a -> target_a
    /// - Old vote: source_b -> target_b
    /// - source_a < source_b < target_b < target_a (outer surrounds inner)
    pub fn check_surround_vote(
        &mut self,
        validator: ValidatorId,
        source: u64,
        target: u64,
        _votes: &[(u64, u64)], // (source, target) pairs from history
    ) -> Option<SlashingEvidence> {
        // TODO: Full Casper FFG slashing detection
        // For now, focus on double-vote which is more common
        let _ = (validator, source, target);
        None
    }

    /// Get pending slashings to broadcast.
    pub fn drain_pending(&mut self) -> Vec<SlashingEvidence> {
        std::mem::take(&mut self.pending_slashings)
    }

    /// Get total slashings recorded.
    pub fn total_slashings(&self) -> u64 {
        self.total_slashings
    }

    /// Check if validator has voted at epoch.
    pub fn has_vote(&self, validator: ValidatorId, epoch: u64) -> bool {
        self.votes.contains_key(&(validator, epoch))
    }

    /// Get the vote for a validator at epoch.
    pub fn get_vote(&self, validator: ValidatorId, epoch: u64) -> Option<Hash> {
        self.votes.get(&(validator, epoch)).copied()
    }

    /// Clear old votes before a certain epoch (garbage collection).
    pub fn prune_before(&mut self, epoch: u64) {
        self.votes.retain(|(_, e), _| *e >= epoch);
    }

    /// Get stats.
    pub fn stats(&self) -> SlashingStats {
        SlashingStats {
            total_votes: self.votes.len(),
            pending_slashings: self.pending_slashings.len(),
            total_slashings: self.total_slashings,
        }
    }
}

/// Slashing statistics.
#[derive(Clone, Debug)]
pub struct SlashingStats {
    pub total_votes: usize,
    pub pending_slashings: usize,
    pub total_slashings: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validator(id: u8) -> ValidatorId {
        [id; 32]
    }

    fn block_hash(id: u8) -> Hash {
        [id; 32]
    }

    #[test]
    fn test_first_vote_no_slashing() {
        let mut db = SlashingDB::new();

        let result = db.check_and_record(validator(1), 10, block_hash(0xAB), None);

        assert!(result.is_none());
        assert!(db.has_vote(validator(1), 10));
    }

    #[test]
    fn test_same_vote_no_slashing() {
        let mut db = SlashingDB::new();

        db.check_and_record(validator(1), 10, block_hash(0xAB), None);
        let result = db.check_and_record(validator(1), 10, block_hash(0xAB), None);

        assert!(result.is_none());
        assert_eq!(db.total_slashings(), 0);
    }

    #[test]
    fn test_double_vote_triggers_slashing() {
        let mut db = SlashingDB::new();

        db.check_and_record(validator(1), 10, block_hash(0xAB), None);
        let result = db.check_and_record(validator(1), 10, block_hash(0xCD), None);

        assert!(result.is_some());
        let evidence = result.unwrap();

        match evidence {
            SlashingEvidence::DoubleVote {
                validator: v,
                epoch,
                vote_a,
                vote_b,
                ..
            } => {
                assert_eq!(v, validator(1));
                assert_eq!(epoch, 10);
                assert_eq!(vote_a, block_hash(0xAB));
                assert_eq!(vote_b, block_hash(0xCD));
            }
            _ => panic!("Expected DoubleVote"),
        }

        assert_eq!(db.total_slashings(), 1);
    }

    #[test]
    fn test_different_epochs_no_slashing() {
        let mut db = SlashingDB::new();

        db.check_and_record(validator(1), 10, block_hash(0xAB), None);
        let result = db.check_and_record(validator(1), 11, block_hash(0xCD), None);

        assert!(result.is_none());
        assert_eq!(db.total_slashings(), 0);
    }

    #[test]
    fn test_prune_old_epochs() {
        let mut db = SlashingDB::new();

        db.check_and_record(validator(1), 5, block_hash(0xAA), None);
        db.check_and_record(validator(1), 10, block_hash(0xBB), None);
        db.check_and_record(validator(1), 15, block_hash(0xCC), None);

        db.prune_before(10);

        assert!(!db.has_vote(validator(1), 5));
        assert!(db.has_vote(validator(1), 10));
        assert!(db.has_vote(validator(1), 15));
    }
}
