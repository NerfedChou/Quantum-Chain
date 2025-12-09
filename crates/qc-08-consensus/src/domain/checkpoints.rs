//! # Weak Subjectivity Checkpoints
//!
//! Defense against long-range attacks using social consensus anchors.
//!
//! ## Threat: Long-Range Attack
//!
//! An attacker buys old validator keys (from validators who have withdrawn).
//! Using these keys, they rewrite the chain from months ago. To a new node,
//! this fake chain looks valid with higher total weight.
//!
//! ## Solution: Anchor-Point Rejection
//!
//! 1. Hardcode a checkpoint (hash + height) into node software
//! 2. Any chain not containing this checkpoint is INVALID
//! 3. New nodes must trust the checkpoint from a trusted source
//!
//! Reference: SPEC-08-CONSENSUS.md Phase 1 Security

use shared_types::Hash;

/// Weak subjectivity checkpoint.
///
/// A social consensus anchor that nodes must trust to bootstrap safely.
#[derive(Clone, Debug, PartialEq)]
pub struct WeakSubjectivityCheckpoint {
    /// Block hash of the checkpoint
    pub block_hash: Hash,
    /// Block height of the checkpoint
    pub block_height: u64,
    /// Epoch number
    pub epoch: u64,
    /// State root at this checkpoint
    pub state_root: Hash,
}

impl WeakSubjectivityCheckpoint {
    pub fn new(block_hash: Hash, block_height: u64, epoch: u64, state_root: Hash) -> Self {
        Self {
            block_hash,
            block_height,
            epoch,
            state_root,
        }
    }

    /// Genesis checkpoint (block 0).
    pub fn genesis(genesis_hash: Hash) -> Self {
        Self {
            block_hash: genesis_hash,
            block_height: 0,
            epoch: 0,
            state_root: [0u8; 32],
        }
    }
}

/// Weak subjectivity configuration.
#[derive(Clone, Debug)]
pub struct WeakSubjectivityConfig {
    /// Currently trusted checkpoint
    pub checkpoint: Option<WeakSubjectivityCheckpoint>,
    /// Maximum age of checkpoint in epochs before requiring new one
    pub max_age_epochs: u64,
    /// Enforce checkpoint validation
    pub enforce: bool,
}

impl Default for WeakSubjectivityConfig {
    fn default() -> Self {
        Self {
            checkpoint: None,
            max_age_epochs: 1024, // ~4.5 days with 12s slots, 32 slots/epoch
            enforce: true,
        }
    }
}

/// Weak subjectivity validator.
#[derive(Debug)]
pub struct WeakSubjectivityValidator {
    config: WeakSubjectivityConfig,
}

impl WeakSubjectivityValidator {
    pub fn new(config: WeakSubjectivityConfig) -> Self {
        Self { config }
    }

    /// Validate that a chain contains the required checkpoint.
    ///
    /// Returns `Ok(())` if:
    /// - No checkpoint configured (enforcement disabled)
    /// - Chain contains the checkpoint block
    ///
    /// Returns `Err(WeakSubjectivityError)` if:
    /// - Checkpoint configured but chain doesn't contain it
    pub fn validate_chain<F>(&self, chain_contains: F) -> Result<(), WeakSubjectivityError>
    where
        F: Fn(&Hash) -> bool,
    {
        if !self.config.enforce {
            return Ok(());
        }

        match &self.config.checkpoint {
            None => Ok(()),
            Some(checkpoint) => {
                if chain_contains(&checkpoint.block_hash) {
                    Ok(())
                } else {
                    Err(WeakSubjectivityError::CheckpointNotFound {
                        expected_hash: checkpoint.block_hash,
                        expected_height: checkpoint.block_height,
                    })
                }
            }
        }
    }

    /// Check if checkpoint is too old.
    pub fn is_checkpoint_stale(&self, current_epoch: u64) -> bool {
        match &self.config.checkpoint {
            None => false,
            Some(checkpoint) => {
                current_epoch.saturating_sub(checkpoint.epoch) > self.config.max_age_epochs
            }
        }
    }

    /// Get the checkpoint block hash.
    pub fn checkpoint_hash(&self) -> Option<Hash> {
        self.config.checkpoint.as_ref().map(|c| c.block_hash)
    }

    /// Update the checkpoint.
    pub fn set_checkpoint(&mut self, checkpoint: WeakSubjectivityCheckpoint) {
        self.config.checkpoint = Some(checkpoint);
    }
}

/// Weak subjectivity errors.
#[derive(Clone, Debug, PartialEq)]
pub enum WeakSubjectivityError {
    /// Chain does not contain required checkpoint
    CheckpointNotFound {
        expected_hash: Hash,
        expected_height: u64,
    },
    /// Checkpoint is too old
    CheckpointStale { age_epochs: u64, max_epochs: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkpoint_hash() -> Hash {
        [0xAB; 32]
    }

    fn other_hash() -> Hash {
        [0xCD; 32]
    }

    #[test]
    fn test_no_checkpoint_always_valid() {
        let config = WeakSubjectivityConfig {
            checkpoint: None,
            enforce: true,
            ..Default::default()
        };
        let validator = WeakSubjectivityValidator::new(config);
        
        let result = validator.validate_chain(|_| false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_chain_contains_checkpoint_valid() {
        let config = WeakSubjectivityConfig {
            checkpoint: Some(WeakSubjectivityCheckpoint::new(
                checkpoint_hash(),
                1000,
                100,
                [0; 32],
            )),
            enforce: true,
            ..Default::default()
        };
        let validator = WeakSubjectivityValidator::new(config);
        
        let result = validator.validate_chain(|h| *h == checkpoint_hash());
        assert!(result.is_ok());
    }

    #[test]
    fn test_chain_missing_checkpoint_invalid() {
        let config = WeakSubjectivityConfig {
            checkpoint: Some(WeakSubjectivityCheckpoint::new(
                checkpoint_hash(),
                1000,
                100,
                [0; 32],
            )),
            enforce: true,
            ..Default::default()
        };
        let validator = WeakSubjectivityValidator::new(config);
        
        let result = validator.validate_chain(|h| *h == other_hash());
        assert!(matches!(result, Err(WeakSubjectivityError::CheckpointNotFound { .. })));
    }

    #[test]
    fn test_checkpoint_staleness() {
        let config = WeakSubjectivityConfig {
            checkpoint: Some(WeakSubjectivityCheckpoint::new(
                checkpoint_hash(),
                1000,
                100,
                [0; 32],
            )),
            max_age_epochs: 1024,
            enforce: true,
        };
        let validator = WeakSubjectivityValidator::new(config);
        
        // Not stale
        assert!(!validator.is_checkpoint_stale(500));
        
        // Stale
        assert!(validator.is_checkpoint_stale(2000));
    }
}
