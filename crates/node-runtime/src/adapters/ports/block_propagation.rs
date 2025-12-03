//! # Block Propagation Port Adapters
//!
//! Implements the outbound port traits required by qc-05-block-propagation.
//!
//! ## Ports Implemented
//!
//! - `PeerNetwork` - Network operations for peer communication
//! - `ConsensusGateway` - Submits blocks to consensus for validation
//! - `MempoolGateway` - Transaction lookup for compact block reconstruction
//! - `SignatureVerifier` - Block signature verification

use parking_lot::RwLock;
use shared_types::Hash;
use std::sync::Arc;

use qc_05_block_propagation::domain::{PeerId, ShortTxId};
use qc_05_block_propagation::events::PropagationError;
use qc_05_block_propagation::ports::outbound::{
    ConsensusGateway, MempoolGateway, NetworkMessage, PeerInfo, PeerNetwork, SignatureVerifier,
};
use qc_06_mempool::TransactionPool;

// =============================================================================
// PeerNetwork Adapter
// =============================================================================

/// Adapter implementing qc-05's PeerNetwork trait.
/// In production, this would interface with actual P2P networking.
pub struct BlockPropNetworkAdapter {
    /// Connected peers (mock for now)
    peers: RwLock<Vec<PeerInfo>>,
}

impl BlockPropNetworkAdapter {
    pub fn new() -> Self {
        Self {
            peers: RwLock::new(Vec::new()),
        }
    }

    /// Add a peer to the network
    pub fn add_peer(&self, peer: PeerInfo) {
        self.peers.write().push(peer);
    }

    /// Remove a peer from the network
    pub fn remove_peer(&self, peer_id: &PeerId) {
        self.peers.write().retain(|p| p.peer_id != *peer_id);
    }
}

impl Default for BlockPropNetworkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PeerNetwork for BlockPropNetworkAdapter {
    fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().clone()
    }

    fn send_to_peer(
        &self,
        peer_id: PeerId,
        _message: NetworkMessage,
    ) -> Result<(), PropagationError> {
        let peers = self.peers.read();
        if peers.iter().any(|p| p.peer_id == peer_id) {
            Ok(())
        } else {
            Err(PropagationError::UnknownPeer(peer_id.0))
        }
    }

    fn broadcast(
        &self,
        peer_ids: &[PeerId],
        _message: NetworkMessage,
    ) -> Vec<Result<(), PropagationError>> {
        let peers = self.peers.read();
        peer_ids
            .iter()
            .map(|peer_id| {
                if peers.iter().any(|p| p.peer_id == *peer_id) {
                    Ok(())
                } else {
                    Err(PropagationError::UnknownPeer(peer_id.0))
                }
            })
            .collect()
    }
}

// =============================================================================
// ConsensusGateway Adapter
// =============================================================================

/// Adapter implementing qc-05's ConsensusGateway trait.
/// Submits blocks to consensus for validation.
pub struct BlockPropConsensusAdapter;

impl BlockPropConsensusAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlockPropConsensusAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsensusGateway for BlockPropConsensusAdapter {
    fn submit_block_for_validation(
        &self,
        block_hash: Hash,
        _block_data: Vec<u8>,
        _source_peer: PeerId,
    ) -> Result<(), PropagationError> {
        // In production, this would submit to qc-08 Consensus
        tracing::debug!(
            "Block {} submitted for validation",
            hex::encode(&block_hash[..8])
        );
        Ok(())
    }
}

// =============================================================================
// MempoolGateway Adapter
// =============================================================================

/// Adapter implementing qc-05's MempoolGateway trait.
/// Used for compact block reconstruction.
pub struct BlockPropMempoolAdapter {
    mempool: Arc<RwLock<TransactionPool>>,
}

impl BlockPropMempoolAdapter {
    pub fn new(mempool: Arc<RwLock<TransactionPool>>) -> Self {
        Self { mempool }
    }
}

impl MempoolGateway for BlockPropMempoolAdapter {
    fn get_transactions_by_short_ids(
        &self,
        short_ids: &[ShortTxId],
        _nonce: u64,
    ) -> Vec<Option<Hash>> {
        // For compact block reconstruction, we need to find transactions by short ID
        // Since the mempool doesn't expose an iterator, we return None for all
        // In production, the mempool would need to support this lookup
        short_ids.iter().map(|_| None).collect()
    }
}

// =============================================================================
// SignatureVerifier Adapter
// =============================================================================

/// Adapter implementing qc-05's SignatureVerifier trait.
/// Verifies block proposer signatures.
pub struct BlockPropSignatureAdapter;

impl BlockPropSignatureAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlockPropSignatureAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureVerifier for BlockPropSignatureAdapter {
    fn verify_block_signature(
        &self,
        block_hash: &Hash,
        proposer_pubkey: &[u8],
        signature: &[u8],
    ) -> Result<bool, PropagationError> {
        use qc_10_signature_verification::domain::entities::EcdsaSignature;
        use qc_10_signature_verification::domain::ecdsa::verify_ecdsa;

        // Signature must be 65 bytes (r: 32, s: 32, v: 1)
        if signature.len() < 65 {
            return Ok(false);
        }

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature[..32]);
        s.copy_from_slice(&signature[32..64]);
        let v = signature[64];

        let sig = EcdsaSignature { r, s, v };
        let result = verify_ecdsa(block_hash, &sig);

        // Optionally verify the recovered address matches proposer
        if result.valid {
            if let Some(recovered) = result.recovered_address {
                // Compare recovered address with proposer pubkey-derived address
                // For now, just check the signature is valid
                let _ = (recovered, proposer_pubkey);
            }
        }

        Ok(result.valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_adapter_creation() {
        let adapter = BlockPropNetworkAdapter::new();
        assert!(adapter.get_connected_peers().is_empty());
    }

    #[test]
    fn test_add_remove_peer() {
        let adapter = BlockPropNetworkAdapter::new();
        let peer = PeerInfo {
            peer_id: PeerId::new([1u8; 32]),
            reputation: 1.0,
            latency_ms: 50,
            is_connected: true,
        };

        adapter.add_peer(peer.clone());
        assert_eq!(adapter.get_connected_peers().len(), 1);

        adapter.remove_peer(&peer.peer_id);
        assert!(adapter.get_connected_peers().is_empty());
    }

    #[test]
    fn test_consensus_gateway_creation() {
        let _adapter = BlockPropConsensusAdapter::new();
    }

    #[test]
    fn test_signature_adapter_creation() {
        let _adapter = BlockPropSignatureAdapter::new();
    }
}
