//! Driving Ports (API - Inbound)
//!
//! Reference: SPEC-09-FINALITY.md Section 3.1

use crate::domain::{Attestation, Checkpoint, FinalityState, ValidatorId};
use crate::error::FinalityResult;
use async_trait::async_trait;
use shared_types::Hash;

/// Result of processing attestations
#[derive(Clone, Debug)]
pub struct AttestationResult {
    /// Number of accepted attestations
    pub accepted: usize,
    /// Number of rejected attestations
    pub rejected: usize,
    /// Newly justified checkpoint (if any)
    pub new_justified: Option<Checkpoint>,
    /// Newly finalized checkpoint (if any)
    pub new_finalized: Option<Checkpoint>,
}

impl AttestationResult {
    pub fn empty() -> Self {
        Self {
            accepted: 0,
            rejected: 0,
            new_justified: None,
            new_finalized: None,
        }
    }
}

/// Slashable offense type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashableOffenseType {
    DoubleVote,
    SurroundVote,
}

/// Slashable offense info
#[derive(Clone, Debug)]
pub struct SlashableOffenseInfo {
    pub validator_id: ValidatorId,
    pub offense_type: SlashableOffenseType,
    pub detected_epoch: u64,
}

/// Primary Finality API
///
/// Reference: SPEC-09-FINALITY.md Section 3.1
///
/// This is the driving port for the Finality subsystem.
/// It receives attestations and determines justification/finalization.
#[async_trait]
pub trait FinalityApi: Send + Sync {
    /// Process attestations for a checkpoint
    ///
    /// Reference: SPEC-09-FINALITY.md Zero-Trust - re-verifies all signatures
    ///
    /// # Arguments
    /// * `attestations` - Batch of attestations to process
    ///
    /// # Returns
    /// * Result indicating accepted/rejected counts and any state changes
    async fn process_attestations(
        &self,
        attestations: Vec<Attestation>,
    ) -> FinalityResult<AttestationResult>;

    /// Check if a block is finalized
    async fn is_finalized(&self, block_hash: Hash) -> bool;

    /// Get last finalized checkpoint
    async fn get_last_finalized(&self) -> Option<Checkpoint>;

    /// Get current finality state (circuit breaker)
    async fn get_state(&self) -> FinalityState;

    /// Manual intervention to reset from HALTED state
    ///
    /// Reference: SPEC-09-FINALITY.md Section 1.3
    /// Only operators should call this after investigating failure cause
    async fn reset_from_halted(&self) -> FinalityResult<()>;

    /// Get finality lag (blocks since last finalized)
    async fn get_finality_lag(&self) -> u64;

    /// Get current epoch
    async fn get_current_epoch(&self) -> u64;

    /// Get checkpoint by epoch
    async fn get_checkpoint(&self, epoch: u64) -> Option<Checkpoint>;

    /// Get epochs without finality (for inactivity leak monitoring)
    async fn get_epochs_without_finality(&self) -> u64;

    /// Check if inactivity leak is active
    async fn is_inactivity_leak_active(&self) -> bool;

    /// Get detected slashable offenses
    async fn get_slashable_offenses(&self) -> Vec<SlashableOffenseInfo>;
}
