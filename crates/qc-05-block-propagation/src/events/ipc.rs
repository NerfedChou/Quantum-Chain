//! IPC message types for Block Propagation subsystem.

use shared_types::Hash;

/// Request to propagate a validated block.
///
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug)]
pub struct PropagateBlockRequest {
    pub block_hash: Hash,
    pub block_data: Vec<u8>,
    pub tx_hashes: Vec<Hash>,
}

/// Block received from network notification (to Consensus).
#[derive(Clone, Debug)]
pub struct BlockReceivedNotification {
    pub block_hash: Hash,
    pub block_data: Vec<u8>,
    pub source_peer: [u8; 32],
    pub received_at_ms: u64,
}

/// Request peer list from Subsystem 1.
#[derive(Clone, Debug)]
pub struct GetPeersRequest {
    pub correlation_id: u64,
    pub min_reputation: Option<f64>,
    pub max_count: usize,
}

/// Peer list response from Subsystem 1.
#[derive(Clone, Debug)]
pub struct GetPeersResponse {
    pub correlation_id: u64,
    pub peers: Vec<PeerData>,
}

/// Peer data from Peer Discovery.
#[derive(Clone, Debug)]
pub struct PeerData {
    pub peer_id: [u8; 32],
    pub reputation: f64,
    pub latency_ms: u64,
}
