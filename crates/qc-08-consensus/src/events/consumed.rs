//! Consumed events (Incoming)
//!
//! Reference: SPEC-08-CONSENSUS.md Section 4.2

use crate::domain::{Block, SignedTransaction};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use shared_types::Hash;

/// Correlation ID for request-response tracking
pub type CorrelationId = [u8; 16];

/// Block received from network for validation
///
/// Reference: SPEC-08 Section 4.2
///
/// # Security
/// Envelope sender_id MUST be 5 (Block Propagation)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidateBlockRequest {
    pub correlation_id: CorrelationId,
    pub block: Block,
    pub source_peer: Option<[u8; 32]>,
    /// Timestamp when block was received
    pub received_at: u64,
}

/// Transaction batch from Mempool
///
/// Reference: SPEC-08 Section 4.2
///
/// # Security
/// Envelope sender_id MUST be 6 (Mempool)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionBatchResponse {
    pub correlation_id: CorrelationId,
    pub transactions: Vec<SignedTransaction>,
    pub total_gas: u64,
}

/// Attestation received from another validator
///
/// # Security
/// Envelope sender_id MUST be 10 (Sig Verify)
/// But we ZERO-TRUST re-verify the signature anyway
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttestationReceived {
    pub validator: [u8; 32],
    pub block_hash: Hash,
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 65],
    pub slot: u64,
    pub epoch: u64,
    /// Pre-validation flag from Subsystem 10
    ///
    /// # Security Warning
    /// NEVER trust this flag! Always re-verify independently.
    pub signature_valid: bool,
}

/// PBFT message received
///
/// # Security
/// Envelope sender_id MUST be 10 (Sig Verify)
/// But we ZERO-TRUST re-verify the signature anyway
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PBFTMessageReceived {
    Prepare(PBFTPayload),
    Commit(PBFTPayload),
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PBFTPayload {
    pub view: u64,
    pub sequence: u64,
    pub block_hash: Hash,
    pub validator: [u8; 32],
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 65],
    /// Pre-validation flag - NEVER TRUST THIS
    pub signature_valid: bool,
}
