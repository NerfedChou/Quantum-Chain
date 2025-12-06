//! Inbound events (subscribed)

use crate::domain::VRFProof;
use primitive_types::H256;
use serde::{Deserialize, Serialize};

/// Event from Finality (9): Block finalized, produce next block
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockFinalizedEvent {
    /// Event version
    pub version: u16,

    /// Sender subsystem ID (must be 9)
    pub sender_id: u8,

    /// Finalized block hash
    pub block_hash: H256,

    /// Finalized block number
    pub block_number: u64,

    /// Finalization timestamp
    pub finalized_at: u64,
}

/// Event from Consensus (8): PoS proposer duty assigned
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SlotAssignedEvent {
    /// Event version
    pub version: u16,

    /// Sender subsystem ID (must be 8)
    pub sender_id: u8,

    /// Assigned slot number
    pub slot: u64,

    /// Current epoch
    pub epoch: u64,

    /// Validator index
    pub validator_index: u32,

    /// VRF proof of selection
    pub vrf_proof: VRFProof,
}

/// Event from Mempool (6): New transaction added (optional optimization)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewPendingTransactionEvent {
    /// Event version
    pub version: u16,

    /// Sender subsystem ID (must be 6)
    pub sender_id: u8,

    /// Transaction hash
    pub tx_hash: H256,

    /// Gas price
    pub gas_price: String, // U256 as string

    /// Gas limit
    pub gas_limit: u64,
}
