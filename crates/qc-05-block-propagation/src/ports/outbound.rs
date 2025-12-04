//! Outbound ports (SPI) for Block Propagation subsystem.

use crate::domain::{PeerId, ShortTxId};
use crate::events::PropagationError;
use shared_types::Hash;

/// Peer network interface for P2P communication.
pub trait PeerNetwork: Send + Sync {
    /// Get list of connected peers.
    fn get_connected_peers(&self) -> Vec<PeerInfo>;

    /// Send message to a specific peer.
    fn send_to_peer(
        &self,
        peer_id: PeerId,
        message: NetworkMessage,
    ) -> Result<(), PropagationError>;

    /// Broadcast message to multiple peers.
    fn broadcast(
        &self,
        peer_ids: &[PeerId],
        message: NetworkMessage,
    ) -> Vec<Result<(), PropagationError>>;
}

/// Peer information.
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub reputation: f64,
    pub latency_ms: u64,
    pub is_connected: bool,
}

/// Network message types.
#[derive(Clone, Debug)]
pub enum NetworkMessage {
    /// Block announcement (header-first)
    Announce {
        block_hash: Hash,
        block_height: u64,
        parent_hash: Hash,
    },
    /// Compact block
    CompactBlock { data: Vec<u8> },
    /// Full block request
    GetBlock { block_hash: Hash, request_id: u64 },
    /// Full block response
    Block {
        request_id: u64,
        block_data: Option<Vec<u8>>,
    },
    /// Request missing transactions
    GetBlockTxn { block_hash: Hash, indices: Vec<u16> },
    /// Missing transactions response
    BlockTxn {
        block_hash: Hash,
        transactions: Vec<Vec<u8>>,
    },
}

/// Consensus gateway for submitting received blocks.
pub trait ConsensusGateway: Send + Sync {
    /// Submit a received block for validation.
    fn submit_block_for_validation(
        &self,
        block_hash: Hash,
        block_data: Vec<u8>,
        source_peer: PeerId,
    ) -> Result<(), PropagationError>;
}

/// Mempool gateway for compact block reconstruction.
pub trait MempoolGateway: Send + Sync {
    /// Get transactions by short IDs for compact block reconstruction.
    fn get_transactions_by_short_ids(
        &self,
        short_ids: &[ShortTxId],
        nonce: u64,
    ) -> Vec<Option<Hash>>;
}

/// Signature verification gateway.
///
/// Reference: IPC-MATRIX.md, Subsystem 10 - Block Propagation listed
/// in "Who Is Allowed To Talk To Me"
pub trait SignatureVerifier: Send + Sync {
    /// Verify block proposer signature.
    ///
    /// Security Note: Invalid signatures result in SILENT DROP, not ban.
    /// Reference: Architecture.md - IP spoofing defense
    fn verify_block_signature(
        &self,
        block_hash: &Hash,
        proposer_pubkey: &[u8],
        signature: &[u8],
    ) -> Result<bool, PropagationError>;
}
