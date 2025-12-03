//! Outgoing events for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 4.1

use crate::domain::proof::FinalityProof;
use crate::domain::FinalityState;
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
