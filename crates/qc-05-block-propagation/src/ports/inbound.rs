//! Inbound ports (API) for Block Propagation subsystem.

use crate::domain::{PropagationMetrics, PropagationState, PropagationStats};
use crate::events::PropagationError;
use shared_types::Hash;

/// Primary API for block propagation.
///
/// # Security (IPC-MATRIX.md)
///
/// Only Consensus (Subsystem 8) can call `propagate_block`.
pub trait BlockPropagationApi: Send + Sync {
    /// Propagate a validated block to the network.
    ///
    /// Called by Consensus after block validation.
    ///
    /// # Arguments
    /// * `block_hash` - Hash of the validated block
    /// * `block_data` - Serialized block data
    /// * `tx_hashes` - Transaction hashes in the block
    ///
    /// # Returns
    /// Propagation statistics including peers reached
    fn propagate_block(
        &self,
        block_hash: Hash,
        block_data: Vec<u8>,
        tx_hashes: Vec<Hash>,
    ) -> Result<PropagationStats, PropagationError>;

    /// Get propagation status for a block.
    fn get_propagation_status(
        &self,
        block_hash: Hash,
    ) -> Result<Option<PropagationState>, PropagationError>;

    /// Get network propagation metrics.
    fn get_propagation_metrics(&self) -> PropagationMetrics;
}

/// Handle for receiving blocks from network.
pub trait BlockReceiver: Send + Sync {
    /// Handle incoming block announcement from network peer.
    fn handle_announcement(
        &self,
        peer_id: [u8; 32],
        block_hash: Hash,
        block_height: u64,
    ) -> Result<(), PropagationError>;

    /// Handle incoming compact block from network peer.
    fn handle_compact_block(
        &self,
        peer_id: [u8; 32],
        compact_block_data: Vec<u8>,
    ) -> Result<(), PropagationError>;

    /// Handle incoming full block from network peer.
    fn handle_full_block(
        &self,
        peer_id: [u8; 32],
        block_data: Vec<u8>,
    ) -> Result<(), PropagationError>;
}
