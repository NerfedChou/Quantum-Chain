//! Published events (Outgoing)
//!
//! Reference: SPEC-08-CONSENSUS.md Section 4.1

use crate::domain::{ValidatedBlock, ValidationProof};
use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// V2.3: Published to Event Bus after validating a block
///
/// Reference: SPEC-08 Section 4.1
///
/// Triggers choreography: Subsystems 2, 3, 4 all subscribe
///
/// # Security (Envelope-Only Identity)
/// No requester_id in payload - identity comes from envelope only
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockValidatedEvent {
    /// Block hash (correlation key for assembly)
    pub block_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// The validated block with transactions
    pub block: ValidatedBlock,
    /// Consensus proof (PoS attestations or PBFT votes)
    pub consensus_proof: ValidationProof,
    /// Validation timestamp (unix seconds)
    pub validated_at: u64,
}

impl BlockValidatedEvent {
    /// Create a new BlockValidatedEvent
    pub fn new(block: ValidatedBlock, consensus_proof: ValidationProof, validated_at: u64) -> Self {
        Self {
            block_hash: block.header.hash(),
            block_height: block.header.block_height,
            block,
            consensus_proof,
            validated_at,
        }
    }
}

/// Block propagation request (to Subsystem 5)
///
/// After validating a locally-built block, request propagation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropagateBlockRequest {
    pub block_hash: Hash,
    pub block: ValidatedBlock,
    pub priority: PropagationPriority,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropagationPriority {
    /// Propagate immediately (locally built block)
    High,
    /// Normal propagation (validated network block)
    Normal,
}
