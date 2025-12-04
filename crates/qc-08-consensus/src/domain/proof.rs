//! Validation proof types for PoS and PBFT
//!
//! Reference: SPEC-08-CONSENSUS.md Section 2.1

use super::ValidatorId;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use shared_types::Hash;

/// Validation proof (PoS or PBFT)
///
/// Reference: SPEC-08 Section 2.1
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationProof {
    PoS(PoSProof),
    PBFT(PBFTProof),
}

impl ValidationProof {
    /// Get the epoch this proof belongs to
    pub fn epoch(&self) -> u64 {
        match self {
            ValidationProof::PoS(p) => p.epoch,
            ValidationProof::PBFT(p) => p.epoch,
        }
    }
}

/// Proof of Stake validation proof
///
/// Reference: SPEC-08 Section 2.1
/// System.md: "2/3 validators must attest"
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoSProof {
    /// Aggregated BLS signatures from attesters (simplified as individual sigs for now)
    pub attestations: Vec<Attestation>,
    /// Epoch number
    pub epoch: u64,
    /// Slot number within epoch
    pub slot: u64,
}

impl PoSProof {
    /// Count participating validators
    pub fn participation_count(&self) -> usize {
        self.attestations.len()
    }

    /// Get list of participating validators
    pub fn participating_validators(&self) -> Vec<ValidatorId> {
        self.attestations.iter().map(|a| a.validator).collect()
    }
}

/// A single attestation from a validator
///
/// Supports both ECDSA (65 bytes) and BLS (96 bytes) signatures
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attestation {
    pub validator: ValidatorId,
    pub block_hash: Hash,
    /// Signature bytes (65 for ECDSA, 96 for BLS)
    #[serde_as(as = "Bytes")]
    pub signature: Vec<u8>,
    pub slot: u64,
}

impl Attestation {
    /// Create an attestation with ECDSA signature (65 bytes)
    pub fn new_ecdsa(
        validator: ValidatorId,
        block_hash: Hash,
        signature: [u8; 65],
        slot: u64,
    ) -> Self {
        Self {
            validator,
            block_hash,
            signature: signature.to_vec(),
            slot,
        }
    }

    /// Create an attestation with BLS signature (96 bytes)
    pub fn new_bls(
        validator: ValidatorId,
        block_hash: Hash,
        signature: [u8; 96],
        slot: u64,
    ) -> Self {
        Self {
            validator,
            block_hash,
            signature: signature.to_vec(),
            slot,
        }
    }

    /// Check if this is a BLS signature
    pub fn is_bls(&self) -> bool {
        self.signature.len() == 96
    }
}

/// PBFT validation proof
///
/// Reference: SPEC-08 Section 2.1
/// Requires 2f+1 prepare and commit messages
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PBFTProof {
    /// Prepare messages (2f+1 required)
    pub prepares: Vec<PrepareMessage>,
    /// Commit messages (2f+1 required)
    pub commits: Vec<CommitMessage>,
    /// View number
    pub view: u64,
    /// Epoch number
    pub epoch: u64,
}

impl PBFTProof {
    /// Count prepare messages
    pub fn prepare_count(&self) -> usize {
        self.prepares.len()
    }

    /// Count commit messages
    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    /// Get unique validators who prepared
    pub fn prepare_validators(&self) -> Vec<ValidatorId> {
        self.prepares.iter().map(|p| p.validator).collect()
    }

    /// Get unique validators who committed
    pub fn commit_validators(&self) -> Vec<ValidatorId> {
        self.commits.iter().map(|c| c.validator).collect()
    }
}

/// PBFT Prepare message
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrepareMessage {
    pub view: u64,
    pub sequence: u64,
    pub block_hash: Hash,
    pub validator: ValidatorId,
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 65],
}

/// PBFT Commit message
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitMessage {
    pub view: u64,
    pub sequence: u64,
    pub block_hash: Hash,
    pub validator: ValidatorId,
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 65],
}

/// Message to be signed for attestation
pub fn attestation_signing_message(block_hash: &Hash, slot: u64, epoch: u64) -> Vec<u8> {
    let mut message = Vec::with_capacity(48);
    message.extend_from_slice(block_hash);
    message.extend_from_slice(&slot.to_le_bytes());
    message.extend_from_slice(&epoch.to_le_bytes());
    message
}

/// Message to be signed for PBFT prepare
pub fn prepare_signing_message(view: u64, sequence: u64, block_hash: &Hash) -> Vec<u8> {
    let mut message = Vec::with_capacity(48);
    message.extend_from_slice(b"PREPARE");
    message.extend_from_slice(&view.to_le_bytes());
    message.extend_from_slice(&sequence.to_le_bytes());
    message.extend_from_slice(block_hash);
    message
}

/// Message to be signed for PBFT commit
pub fn commit_signing_message(view: u64, sequence: u64, block_hash: &Hash) -> Vec<u8> {
    let mut message = Vec::with_capacity(48);
    message.extend_from_slice(b"COMMIT");
    message.extend_from_slice(&view.to_le_bytes());
    message.extend_from_slice(&sequence.to_le_bytes());
    message.extend_from_slice(block_hash);
    message
}
