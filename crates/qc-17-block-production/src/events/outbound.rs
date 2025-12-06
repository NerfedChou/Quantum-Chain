//! Outbound events (published)

use crate::domain::ConsensusMode;
use primitive_types::H256;
use serde::{Deserialize, Serialize};

/// Event: Block successfully produced
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockProducedEvent {
    /// Event version
    pub version: u16,

    /// Sender subsystem ID (always 17)
    pub sender_id: u8,

    /// Produced block hash
    pub block_hash: H256,

    /// Block number
    pub block_number: u64,

    /// Transaction count
    pub transaction_count: u32,

    /// Total gas used
    pub total_gas_used: u64,

    /// Total fees collected (as string)
    pub total_fees: String,

    /// Production time in milliseconds
    pub production_time_ms: u64,

    /// Consensus mode used
    pub consensus_mode: ConsensusMode,

    /// Event timestamp
    pub timestamp: u64,
}

/// Event: Mining/proposing metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiningMetrics {
    /// Event version
    pub version: u16,

    /// Sender subsystem ID (always 17)
    pub sender_id: u8,

    // Transaction selection metrics
    /// Transactions considered
    pub transactions_considered: u32,

    /// Transactions selected
    pub transactions_selected: u32,

    /// Total gas used
    pub total_gas_used: u64,

    /// Total fees (as string)
    pub total_fees: String,

    /// Selection time in milliseconds
    pub selection_time_ms: u64,

    // PoW specific
    /// Hashrate in H/s (PoW only)
    pub hashrate: Option<f64>,

    /// Mining time in milliseconds (PoW only)
    pub mining_time_ms: Option<u64>,

    // PoS specific
    /// Slot number (PoS only)
    pub slot_number: Option<u64>,

    // MEV metrics
    /// MEV bundles detected
    pub mev_bundles_detected: u32,

    /// MEV profit (as string)
    pub mev_profit: String,

    /// Event timestamp
    pub timestamp: u64,
}
