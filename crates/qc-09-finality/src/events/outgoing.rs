//! Outgoing events for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 4.1

use crate::domain::proof::FinalityProof;
use crate::domain::{CheckpointId, FinalityState, ValidatorId};
use serde::{Deserialize, Serialize};
use shared_types::Hash;
use uuid::Uuid;

/// Correlation ID for tracking request/response pairs
pub type CorrelationId = Uuid;

/// Payload for MarkFinalizedRequest to Block Storage
///
/// Reference: SPEC-09-FINALITY.md Section 4.1
/// SECURITY: No requester_id in payload (Envelope-Only Identity)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkFinalizedPayload {
    pub correlation_id: CorrelationId,
    pub block_hash: Hash,
    pub block_height: u64,
    pub finalized_epoch: u64,
    pub finality_proof: FinalityProof,
}

impl MarkFinalizedPayload {
    pub fn new(
        correlation_id: CorrelationId,
        block_hash: Hash,
        block_height: u64,
        finalized_epoch: u64,
        finality_proof: FinalityProof,
    ) -> Self {
        Self {
            correlation_id,
            block_hash,
            block_height,
            finalized_epoch,
            finality_proof,
        }
    }
}

/// Event emitted when finality is achieved
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityAchievedEvent {
    pub epoch: u64,
    pub block_hash: Hash,
    pub block_height: u64,
    pub participating_stake_percent: u8,
}

/// Event emitted when circuit breaker state changes
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitBreakerStateChangeEvent {
    pub previous_state: String,
    pub new_state: String,
    pub reason: String,
}

impl CircuitBreakerStateChangeEvent {
    pub fn new(previous: FinalityState, new: FinalityState, reason: &str) -> Self {
        Self {
            previous_state: format!("{:?}", previous),
            new_state: format!("{:?}", new),
            reason: reason.to_string(),
        }
    }
}

// =============================================================================
// SLASHING EVENTS
// =============================================================================

/// Type of slashable offense
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashableOffenseType {
    /// Same target epoch, different target block (equivocation)
    DoubleVote,
    /// One attestation surrounds another (FFG slashing condition)
    SurroundVote,
}

/// Event emitted when a slashable offense is detected
///
/// Reference: SPEC-09 INVARIANT-3 - No conflicting finality without slashing 1/3 validators
///
/// This event should be consumed by an enforcement subsystem to:
/// 1. Slash the validator's stake
/// 2. Remove validator from active set
/// 3. Potentially initiate withdrawal delay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlashableOffenseDetectedEvent {
    /// The validator who committed the offense
    pub validator_id: ValidatorId,
    /// Type of offense (DoubleVote or SurroundVote)
    pub offense_type: SlashableOffenseType,
    /// First attestation (evidence)
    pub attestation1_source: CheckpointId,
    pub attestation1_target: CheckpointId,
    /// Second attestation (evidence)
    pub attestation2_source: CheckpointId,
    pub attestation2_target: CheckpointId,
    /// Epoch when offense was detected
    pub detected_epoch: u64,
    /// Recommended slash amount (percentage of stake, e.g., 100 = 100%)
    pub recommended_slash_percent: u8,
}

/// Evidence of conflicting attestations for a slashable offense.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashingEvidence {
    /// First attestation source checkpoint
    pub att1_source: CheckpointId,
    /// First attestation target checkpoint
    pub att1_target: CheckpointId,
    /// Second attestation source checkpoint
    pub att2_source: CheckpointId,
    /// Second attestation target checkpoint
    pub att2_target: CheckpointId,
}

impl SlashableOffenseDetectedEvent {
    /// Create a new slashing event
    ///
    /// Slash amounts per Casper FFG:
    /// - DoubleVote: 100% of stake (severe - direct equivocation)
    /// - SurroundVote: 100% of stake (severe - FFG safety violation)
    pub fn new(
        validator_id: ValidatorId,
        offense_type: SlashableOffenseType,
        evidence: SlashingEvidence,
        detected_epoch: u64,
    ) -> Self {
        // Both offense types warrant full slashing per Casper FFG
        let recommended_slash_percent = 100;

        Self {
            validator_id,
            offense_type,
            attestation1_source: evidence.att1_source,
            attestation1_target: evidence.att1_target,
            attestation2_source: evidence.att2_source,
            attestation2_target: evidence.att2_target,
            detected_epoch,
            recommended_slash_percent,
        }
    }
}

// =============================================================================
// INACTIVITY LEAK EVENTS
// =============================================================================

/// Event emitted when inactivity leak is triggered
///
/// Reference: SPEC-09 - inactivity_leak_epochs configuration
///
/// When finality hasn't been achieved for `inactivity_leak_epochs` consecutive
/// epochs, inactive validators gradually lose stake to incentivize participation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InactivityLeakTriggeredEvent {
    /// Epoch when leak was triggered
    pub epoch: u64,
    /// Number of consecutive epochs without finality
    pub epochs_without_finality: u64,
    /// Leak rate per epoch (basis points, e.g., 100 = 1%)
    pub leak_rate_bps: u32,
}

impl InactivityLeakTriggeredEvent {
    pub fn new(epoch: u64, epochs_without_finality: u64, leak_rate_bps: u32) -> Self {
        Self {
            epoch,
            epochs_without_finality,
            leak_rate_bps,
        }
    }
}

/// Event emitted for individual validator stake reduction due to inactivity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorInactivityPenaltyEvent {
    /// Validator being penalized
    pub validator_id: ValidatorId,
    /// Epoch of penalty
    pub epoch: u64,
    /// Amount of stake to be deducted
    pub penalty_amount: u128,
    /// Validator's stake before penalty
    pub stake_before: u128,
    /// Reason (e.g., "no_attestation", "offline")
    pub reason: String,
}
