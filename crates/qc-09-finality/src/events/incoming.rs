//! Incoming events for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 4.2

use crate::domain::Attestation;
use serde::{Deserialize, Serialize};

/// Attestations from Consensus for finality processing
///
/// Reference: SPEC-09-FINALITY.md Section 4.2
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttestationBatch {
    /// Batch of attestations
    pub attestations: Vec<Attestation>,
    /// Epoch for these attestations
    pub epoch: u64,
    /// Slot at which batch was created
    pub slot: u64,
}

impl AttestationBatch {
    pub fn new(attestations: Vec<Attestation>, epoch: u64, slot: u64) -> Self {
        Self {
            attestations,
            epoch,
            slot,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.attestations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.attestations.len()
    }
}

/// Request to check if a block is finalized
///
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityCheckRequest {
    pub block_hash: [u8; 32],
    pub block_height: u64,
}

/// Request for finality proof
///
/// SECURITY: Envelope sender_id MUST be 15 (Cross-Chain)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinalityProofRequest {
    pub block_hash: [u8; 32],
    pub block_height: u64,
}
