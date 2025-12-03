//! # IPC Payloads
//!
//! Event and request payloads for Transaction Indexing IPC.
//!
//! ## SPEC-03 Reference
//!
//! - Section 4.1: BlockValidatedPayload
//! - Section 4.2: MerkleProofRequestPayload, TransactionLocationRequestPayload
//! - Section 4.3: MerkleRootComputedPayload, MerkleProofResponsePayload
//!
//! ## Security (Envelope-Only Identity - V2.2)
//!
//! CRITICAL: These payloads contain NO identity fields (requester_id, sender_id).
//! Sender identity is derived SOLELY from the AuthenticatedMessage envelope.

use serde::{Deserialize, Serialize};
use shared_types::{Hash, ValidatedBlock};

use crate::domain::{IndexingErrorPayload, MerkleProof, TransactionLocation};

// ============================================================
// INCOMING EVENTS (Choreography)
// ============================================================

/// Published by Consensus when a block passes validation.
/// This is the TRIGGER for Merkle tree computation.
///
/// ## SPEC-03 Section 4.1
///
/// ## Security
///
/// MUST only accept from sender_id == SubsystemId::Consensus (8)
///
/// ## V2.2 Choreography Pattern
///
/// This event triggers the subsystem to:
/// 1. Compute Merkle tree from block transactions
/// 2. Index all transactions
/// 3. Publish MerkleRootComputed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockValidatedPayload {
    /// The validated block containing transactions to index.
    pub block: ValidatedBlock,
    /// Block hash for correlation.
    pub block_hash: Hash,
    /// Block height for ordering.
    pub block_height: u64,
}

// ============================================================
// INCOMING REQUESTS
// ============================================================

/// Request for a Merkle proof.
///
/// ## SPEC-03 Section 4.2
///
/// ## Security (Envelope-Only Identity)
///
/// NO requester_id field. Sender verified via envelope.sender_id.
/// Response sent to envelope.reply_to with same envelope.correlation_id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProofRequestPayload {
    /// Hash of the transaction to generate proof for.
    pub transaction_hash: Hash,
}

/// Request for transaction location.
///
/// ## SPEC-03 Section 4.2
///
/// ## Security (Envelope-Only Identity)
///
/// NO requester_id field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionLocationRequestPayload {
    /// Hash of the transaction to locate.
    pub transaction_hash: Hash,
}

// ============================================================
// OUTGOING EVENTS (Choreography)
// ============================================================

/// Published after computing Merkle root for a validated block.
///
/// ## SPEC-03 Section 4.3
///
/// ## V2.2 Choreography Pattern
///
/// This is a critical event in the block processing flow.
/// Block Storage (Subsystem 2) buffers this event by block_hash and waits
/// for BlockValidated and StateRootComputed to complete the assembly.
///
/// ## Timing Constraint
///
/// If this event is not emitted within 30 seconds of BlockValidated,
/// Block Storage will time out the assembly for this block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleRootComputedPayload {
    /// Block hash this Merkle root corresponds to.
    pub block_hash: Hash,
    /// Block height for ordering.
    pub block_height: u64,
    /// The computed Merkle root.
    pub merkle_root: Hash,
    /// Number of transactions in the block.
    pub transaction_count: usize,
}

// ============================================================
// OUTGOING RESPONSES
// ============================================================

/// Response to a Merkle proof request.
///
/// ## SPEC-03 Section 4.3
///
/// The correlation_id in the envelope links this to the original request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerkleProofResponsePayload {
    /// The transaction hash that was queried.
    pub transaction_hash: Hash,
    /// The generated proof (if successful).
    pub proof: Option<MerkleProof>,
    /// Error details (if failed).
    pub error: Option<IndexingErrorPayload>,
}

impl MerkleProofResponsePayload {
    /// Create a success response with proof.
    pub fn success(transaction_hash: Hash, proof: MerkleProof) -> Self {
        Self {
            transaction_hash,
            proof: Some(proof),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(transaction_hash: Hash, error: IndexingErrorPayload) -> Self {
        Self {
            transaction_hash,
            proof: None,
            error: Some(error),
        }
    }
}

/// Response to a transaction location request.
///
/// ## SPEC-03 Section 4.3
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionLocationResponsePayload {
    /// The transaction hash that was queried.
    pub transaction_hash: Hash,
    /// The location (if found).
    pub location: Option<TransactionLocation>,
    /// Error details (if failed).
    pub error: Option<IndexingErrorPayload>,
}

impl TransactionLocationResponsePayload {
    /// Create a success response with location.
    pub fn success(transaction_hash: Hash, location: TransactionLocation) -> Self {
        Self {
            transaction_hash,
            location: Some(location),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(transaction_hash: Hash, error: IndexingErrorPayload) -> Self {
        Self {
            transaction_hash,
            location: None,
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash_from_byte(b: u8) -> Hash {
        let mut h = [0u8; 32];
        h[0] = b;
        h
    }

    #[test]
    fn test_merkle_proof_response_success() {
        let tx_hash = hash_from_byte(0x01);
        let proof = MerkleProof {
            leaf_hash: tx_hash,
            tx_index: 0,
            block_height: 100,
            block_hash: hash_from_byte(0xFF),
            root: hash_from_byte(0xAA),
            path: vec![],
        };

        let response = MerkleProofResponsePayload::success(tx_hash, proof.clone());

        assert!(response.proof.is_some());
        assert!(response.error.is_none());
        assert_eq!(response.proof.unwrap().leaf_hash, tx_hash);
    }

    #[test]
    fn test_merkle_proof_response_error() {
        let tx_hash = hash_from_byte(0x01);
        let error = IndexingErrorPayload {
            error_type: crate::domain::IndexingErrorType::TransactionNotFound,
            message: "Not found".to_string(),
            transaction_hash: Some(tx_hash),
            block_hash: None,
        };

        let response = MerkleProofResponsePayload::error(tx_hash, error);

        assert!(response.proof.is_none());
        assert!(response.error.is_some());
    }

    #[test]
    fn test_payloads_have_no_identity_fields() {
        // Compile-time verification: these structs have no sender_id/requester_id
        let _ = MerkleProofRequestPayload {
            transaction_hash: hash_from_byte(0x01),
        };
        let _ = TransactionLocationRequestPayload {
            transaction_hash: hash_from_byte(0x01),
        };
        // If these compile, the structs correctly omit identity fields
    }
}
