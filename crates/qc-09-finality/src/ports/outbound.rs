//! Driven Ports (SPI - Outbound Dependencies)
//!
//! Reference: SPEC-09-FINALITY.md Section 3.2

use crate::domain::proof::FinalityProof;
use crate::domain::{AggregatedAttestations, Attestation, ValidatorId, ValidatorSet};
use crate::error::FinalityResult;
use async_trait::async_trait;
use shared_types::Hash;
use uuid::Uuid;

/// Correlation ID for tracking request/response pairs
pub type CorrelationId = Uuid;

/// Block Storage interface for marking finalized blocks
///
/// Reference: SPEC-09-FINALITY.md Section 3.2
/// Sends MarkFinalizedRequest to Block Storage (Subsystem 2)
#[async_trait]
pub trait BlockStorageGateway: Send + Sync {
    /// Mark a block as finalized
    async fn mark_finalized(&self, request: MarkFinalizedRequest) -> FinalityResult<()>;
}

/// Request to mark a block as finalized
///
/// Reference: SPEC-09-FINALITY.md Section 4.1
/// SECURITY: No requester_id in payload (Envelope-Only Identity)
#[derive(Clone, Debug)]
pub struct MarkFinalizedRequest {
    pub correlation_id: CorrelationId,
    pub block_hash: Hash,
    pub block_height: u64,
    pub finalized_epoch: u64,
    pub finality_proof: FinalityProof,
}

/// Signature verification for attestations
///
/// Reference: SPEC-09-FINALITY.md Section 3.2
/// Uses BLS12-381 signature verification
#[async_trait]
pub trait AttestationVerifier: Send + Sync {
    /// Verify a single attestation signature
    ///
    /// Reference: SPEC-09-FINALITY.md Zero-Trust
    /// Every signature is re-verified independently
    fn verify_attestation(&self, attestation: &Attestation) -> bool;

    /// Verify aggregate BLS signature
    fn verify_aggregate(
        &self,
        attestations: &AggregatedAttestations,
        validators: &ValidatorSet,
    ) -> bool;
}

/// Validator set provider with stake information
///
/// Reference: SPEC-09-FINALITY.md Section 3.2
/// Reference: IPC-MATRIX.md - State Management (4) is authoritative for stake
///
/// CRITICAL: Finality calculations require ACCURATE stake information.
/// Stale stake data could lead to incorrect finalization.
#[async_trait]
pub trait ValidatorSetProvider: Send + Sync {
    /// Get validator set at a specific epoch
    ///
    /// Reference: Architecture.md §5.4.1 - Deterministic state queries
    async fn get_validator_set_at_epoch(&self, epoch: u64) -> FinalityResult<ValidatorSet>;

    /// Get individual validator stake
    ///
    /// Used during zero-trust signature verification to weight attestations
    async fn get_validator_stake(
        &self,
        validator_id: &ValidatorId,
        epoch: u64,
    ) -> FinalityResult<u128>;

    /// Get total active stake at epoch
    ///
    /// Used to calculate 2/3 threshold for justification
    async fn get_total_active_stake(&self, epoch: u64) -> FinalityResult<u128>;
}
