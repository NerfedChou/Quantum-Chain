//! # IPC Message Payloads
//!
//! Defines all IPC message payloads as specified in IPC-MATRIX.md.
//!
//! ## Design Rules (Architecture.md v2.2)
//!
//! - All payloads are wrapped in `AuthenticatedMessage<T>`.
//! - Payloads MUST NOT contain `requester_id` fields (envelope authority).
//! - Request/response pairs use the envelope's `correlation_id`.

use crate::entities::*;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

// =============================================================================
// SUBSYSTEM 1: PEER DISCOVERY
// =============================================================================

/// Request to verify a node's identity.
/// Sender: Subsystem 1 | Receiver: Subsystem 10
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyNodeIdentityPayload {
    /// The node ID to verify.
    pub node_id: NodeId,
    /// The claimed public key.
    pub public_key: PublicKey,
    /// The challenge nonce.
    pub challenge: [u8; 32],
    /// Signature over the challenge.
    #[serde_as(as = "Bytes")]
    pub signature: Signature,
}

/// Response to node identity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyNodeIdentityResponse {
    /// Whether the identity is valid.
    pub valid: bool,
    /// Optional reason for rejection.
    pub reason: Option<String>,
}

/// Request for a list of peers.
/// Uses correlation_id for request/response mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerListRequestPayload {
    /// Maximum number of peers to return.
    pub max_peers: u32,
    /// Optional filter by minimum reputation.
    pub min_reputation: Option<u8>,
}

/// Response containing peer list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerListResponsePayload {
    /// The list of peers.
    pub peers: Vec<PeerInfo>,
}

// =============================================================================
// SUBSYSTEM 2: BLOCK STORAGE (Choreography Pattern)
// =============================================================================

/// Event emitted when a block is validated by Consensus.
/// Triggers parallel computation by Subsystems 3 and 4.
/// Sender: Subsystem 8 | Receivers: Subsystems 2, 3, 4
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockValidatedPayload {
    /// The validated block.
    pub block: ValidatedBlock,
}

/// Event emitted when Merkle root is computed.
/// Sender: Subsystem 3 | Receiver: Subsystem 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleRootComputedPayload {
    /// The block hash this root applies to.
    pub block_hash: Hash,
    /// The computed Merkle root.
    pub merkle_root: Hash,
}

/// Event emitted when State root is computed.
/// Sender: Subsystem 4 | Receiver: Subsystem 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRootComputedPayload {
    /// The block hash this root applies to.
    pub block_hash: Hash,
    /// The computed State root.
    pub state_root: Hash,
}

/// Event emitted when a block is successfully stored.
/// Sender: Subsystem 2 | Receivers: All interested subsystems
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStoredPayload {
    /// The stored block's height.
    pub block_height: u64,
    /// The stored block's hash.
    pub block_hash: Hash,
}

/// Request to read a block by hash.
/// Uses correlation_id for response routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBlockRequestPayload {
    /// The block hash to retrieve.
    pub block_hash: Hash,
}

/// Request to read a range of blocks (for sync).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBlockRangeRequestPayload {
    /// Starting block height.
    pub start_height: u64,
    /// Maximum number of blocks to return.
    pub limit: u64,
}

/// Response containing a stored block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadBlockResponsePayload {
    /// The stored block, if found.
    pub block: Option<StoredBlock>,
}

/// Request to mark a block as finalized.
/// Sender: Subsystem 9 | Receiver: Subsystem 2
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkFinalizedPayload {
    /// The block height to mark as finalized.
    pub block_height: u64,
    /// The finality proof.
    pub proof: FinalityProof,
}

// =============================================================================
// SUBSYSTEM 6: MEMPOOL (Two-Phase Protocol)
// =============================================================================

/// Propose a batch of transactions for block inclusion.
/// Sender: Subsystem 6 | Receiver: Subsystem 8
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposeTransactionBatchPayload {
    /// The transactions being proposed.
    pub transactions: Vec<ValidatedTransaction>,
}

/// Confirmation that transactions were included in a stored block.
/// Sender: Subsystem 2 | Receiver: Subsystem 6
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStorageConfirmationPayload {
    /// The block hash containing the transactions.
    pub block_hash: Hash,
    /// Hashes of transactions that were included.
    pub included_transactions: Vec<Hash>,
}

// =============================================================================
// SUBSYSTEM 10: SIGNATURE VERIFICATION
// =============================================================================

/// Request to verify a signature.
/// Allowed senders: Subsystems 1, 5, 6, 8 (as per IPC-MATRIX)
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySignatureRequestPayload {
    /// The public key.
    pub public_key: PublicKey,
    /// The message that was signed.
    pub message: Vec<u8>,
    /// The signature to verify.
    #[serde_as(as = "Bytes")]
    pub signature: Signature,
}

/// Response to signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySignatureResponsePayload {
    /// Whether the signature is valid.
    pub valid: bool,
}

// =============================================================================
// CRITICAL EVENTS (DLQ Candidates)
// =============================================================================

/// Critical storage error event.
/// Published to DLQ on failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCriticalPayload {
    /// The type of critical error.
    pub error_type: StorageCriticalError,
    /// The block hash involved, if applicable.
    pub block_hash: Option<Hash>,
    /// Human-readable description.
    pub description: String,
}

/// Types of critical storage errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageCriticalError {
    /// Data corruption detected (checksum mismatch).
    DataCorruption,
    /// Disk space below 5% threshold.
    DiskFull,
    /// Database write failure.
    WriteFailed,
    /// Parent block not found (invariant violation).
    ParentNotFound,
}
