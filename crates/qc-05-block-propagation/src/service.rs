//! Block Propagation Service implementation.

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    check_all_invariants, check_rate_limit, create_compact_block, select_peers_for_propagation,
    validate_block_size, InvariantViolation, PeerId, PeerPropagationState, PropagationConfig,
    PropagationMetrics, PropagationState, PropagationStats, SeenBlockCache,
};
use crate::events::PropagationError;
use crate::ports::inbound::{BlockPropagationApi, BlockReceiver};
use crate::ports::outbound::{
    ConsensusGateway, MempoolGateway, NetworkMessage, PeerNetwork, SignatureVerifier,
};
use shared_types::Hash;

/// Block Propagation Service.
///
/// Implements epidemic gossip protocol for block distribution.
#[allow(dead_code)] // Fields used in full implementation
pub struct BlockPropagationService<N, C, M, S>
where
    N: PeerNetwork,
    C: ConsensusGateway,
    M: MempoolGateway,
    S: SignatureVerifier,
{
    config: PropagationConfig,
    seen_cache: Arc<SeenBlockCache>,
    peer_states: RwLock<Vec<PeerPropagationState>>,
    network: Arc<N>,
    consensus: Arc<C>,
    mempool: Arc<M>,
    sig_verifier: Arc<S>,
    metrics: RwLock<PropagationMetrics>,
}

impl<N, C, M, S> BlockPropagationService<N, C, M, S>
where
    N: PeerNetwork,
    C: ConsensusGateway,
    M: MempoolGateway,
    S: SignatureVerifier,
{
    pub fn new(
        config: PropagationConfig,
        network: Arc<N>,
        consensus: Arc<C>,
        mempool: Arc<M>,
        sig_verifier: Arc<S>,
    ) -> Self {
        Self {
            seen_cache: Arc::new(SeenBlockCache::new(config.seen_cache_size)),
            peer_states: RwLock::new(Vec::new()),
            config,
            network,
            consensus,
            mempool,
            sig_verifier,
            metrics: RwLock::new(PropagationMetrics::default()),
        }
    }

    /// Update peer states from network.
    pub fn refresh_peers(&self) {
        let peers = self.network.get_connected_peers();
        let mut states = self.peer_states.write();

        // Add new peers
        for peer in peers {
            if !states.iter().any(|s| s.peer_id == peer.peer_id) {
                let mut state = PeerPropagationState::new(peer.peer_id);
                state.reputation = peer.reputation;
                state.latency_ms = peer.latency_ms;
                states.push(state);
            }
        }
    }

    /// Get current timestamp in milliseconds.
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Find peer state by ID.
    fn find_peer_state(&self, peer_id: &PeerId) -> Option<PeerPropagationState> {
        self.peer_states
            .read()
            .iter()
            .find(|s| s.peer_id == *peer_id)
            .cloned()
    }

    /// Update peer state.
    fn update_peer_state<F>(&self, peer_id: &PeerId, f: F)
    where
        F: FnOnce(&mut PeerPropagationState),
    {
        let mut states = self.peer_states.write();
        if let Some(state) = states.iter_mut().find(|s| s.peer_id == *peer_id) {
            f(state);
        }
    }
}

impl<N, C, M, S> BlockPropagationApi for BlockPropagationService<N, C, M, S>
where
    N: PeerNetwork,
    C: ConsensusGateway,
    M: MempoolGateway,
    S: SignatureVerifier,
{
    fn propagate_block(
        &self,
        block_hash: Hash,
        block_data: Vec<u8>,
        tx_hashes: Vec<Hash>,
    ) -> Result<PropagationStats, PropagationError> {
        let start_time = Self::now_ms();

        // Validate block size
        if !validate_block_size(block_data.len(), &self.config) {
            return Err(PropagationError::BlockTooLarge {
                size: block_data.len(),
                max: self.config.max_block_size_bytes,
            });
        }

        // Check deduplication
        if self.seen_cache.has_seen(&block_hash) {
            return Err(PropagationError::DuplicateBlock(block_hash));
        }

        // Mark as seen
        self.seen_cache.mark_seen(block_hash, None);

        // Refresh peer list
        self.refresh_peers();

        // Select peers for propagation
        let states = self.peer_states.read();
        let selected = select_peers_for_propagation(&states, self.config.fanout);
        drop(states);

        let peer_ids: Vec<PeerId> = selected.iter().map(|s| s.peer_id).collect();

        // Create compact block if enabled
        let message = if self.config.enable_compact_blocks {
            let nonce = rand_nonce();
            let compact = create_compact_block(
                block_hash,
                0,         // Height would come from block header
                [0u8; 32], // Parent hash would come from block header
                Self::now_ms(),
                &tx_hashes,
                nonce,
                &[0], // Prefill coinbase
            );

            // Serialize compact block (simplified)
            NetworkMessage::CompactBlock {
                data: serialize_compact_block(&compact),
            }
        } else {
            NetworkMessage::Block {
                request_id: 0,
                block_data: Some(block_data),
            }
        };

        // Broadcast to selected peers
        let results = self.network.broadcast(&peer_ids, message);
        let peers_reached = results.iter().filter(|r| r.is_ok()).count();

        // Update seen cache state
        self.seen_cache
            .update_state(&block_hash, PropagationState::Complete);

        // Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.blocks_propagated_last_hour += 1;
        }

        Ok(PropagationStats {
            block_hash,
            peers_reached,
            propagation_start_ms: start_time,
            first_ack_time_ms: None,
        })
    }

    fn get_propagation_status(
        &self,
        block_hash: Hash,
    ) -> Result<Option<PropagationState>, PropagationError> {
        Ok(self.seen_cache.get_state(&block_hash))
    }

    fn get_propagation_metrics(&self) -> PropagationMetrics {
        self.metrics.read().clone()
    }
}

impl<N, C, M, S> BlockReceiver for BlockPropagationService<N, C, M, S>
where
    N: PeerNetwork,
    C: ConsensusGateway,
    M: MempoolGateway,
    S: SignatureVerifier,
{
    fn handle_announcement(
        &self,
        peer_id: [u8; 32],
        block_hash: Hash,
        _block_height: u64,
    ) -> Result<(), PropagationError> {
        let peer = PeerId::new(peer_id);

        // Check if peer exists
        let peer_state = self
            .find_peer_state(&peer)
            .ok_or(PropagationError::UnknownPeer(peer_id))?;

        // Check rate limit
        if !check_rate_limit(&peer_state, &self.config) {
            return Err(PropagationError::RateLimited { peer_id });
        }

        // Check deduplication
        if self.seen_cache.has_seen(&block_hash) {
            return Err(PropagationError::DuplicateBlock(block_hash));
        }

        // Record announcement
        self.update_peer_state(&peer, |s| s.record_announcement());

        // Mark block as announced
        self.seen_cache.mark_seen(block_hash, Some(peer));
        self.seen_cache
            .update_state(&block_hash, PropagationState::Announced);

        Ok(())
    }

    fn handle_compact_block(
        &self,
        peer_id: [u8; 32],
        compact_block_data: Vec<u8>,
    ) -> Result<(), PropagationError> {
        let peer = PeerId::new(peer_id);

        // Check if peer exists
        let peer_state = self
            .find_peer_state(&peer)
            .ok_or(PropagationError::UnknownPeer(peer_id))?;

        // Validate size
        if compact_block_data.len() > self.config.max_block_size_bytes {
            return Err(PropagationError::BlockTooLarge {
                size: compact_block_data.len(),
                max: self.config.max_block_size_bytes,
            });
        }

        // Check rate limit
        if !check_rate_limit(&peer_state, &self.config) {
            return Err(PropagationError::RateLimited { peer_id });
        }

        // Deserialize compact block (simplified - would need proper deserialization)
        let block_hash = extract_block_hash(&compact_block_data);

        // Check all invariants
        if let Err(violation) = check_all_invariants(
            &self.seen_cache,
            &block_hash,
            compact_block_data.len(),
            &peer_state,
            &self.config,
        ) {
            return match violation {
                InvariantViolation::DuplicateBlock => {
                    Err(PropagationError::DuplicateBlock(block_hash))
                }
                InvariantViolation::RateLimitExceeded => {
                    Err(PropagationError::RateLimited { peer_id })
                }
                InvariantViolation::BlockTooLarge => Err(PropagationError::BlockTooLarge {
                    size: compact_block_data.len(),
                    max: self.config.max_block_size_bytes,
                }),
            };
        }

        // Record announcement
        self.update_peer_state(&peer, |s| s.record_announcement());

        // Mark as received
        self.seen_cache.mark_seen(block_hash, Some(peer));
        self.seen_cache
            .update_state(&block_hash, PropagationState::CompactReceived);

        // TODO: Reconstruct block from compact + mempool
        // TODO: Verify signature
        // TODO: Submit to consensus

        Ok(())
    }

    fn handle_full_block(
        &self,
        peer_id: [u8; 32],
        block_data: Vec<u8>,
    ) -> Result<(), PropagationError> {
        let peer = PeerId::new(peer_id);

        // Check if peer exists
        let peer_state = self
            .find_peer_state(&peer)
            .ok_or(PropagationError::UnknownPeer(peer_id))?;

        // Validate size
        if !validate_block_size(block_data.len(), &self.config) {
            return Err(PropagationError::BlockTooLarge {
                size: block_data.len(),
                max: self.config.max_block_size_bytes,
            });
        }

        // Check rate limit
        if !check_rate_limit(&peer_state, &self.config) {
            return Err(PropagationError::RateLimited { peer_id });
        }

        // Extract block hash (simplified)
        let block_hash = extract_block_hash(&block_data);

        // Check deduplication
        if !self.seen_cache.can_process(&block_hash) {
            return Err(PropagationError::DuplicateBlock(block_hash));
        }

        // Record announcement
        self.update_peer_state(&peer, |s| s.record_announcement());

        // Mark as complete
        self.seen_cache.mark_seen(block_hash, Some(peer));
        self.seen_cache
            .update_state(&block_hash, PropagationState::Complete);

        // Submit to consensus for validation
        self.consensus
            .submit_block_for_validation(block_hash, block_data, peer)?;

        Ok(())
    }
}

// Helper functions

fn rand_nonce() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

fn serialize_compact_block(compact: &crate::domain::CompactBlock) -> Vec<u8> {
    // Simplified serialization
    let mut data = Vec::new();
    data.extend_from_slice(&compact.header_hash);
    data.extend_from_slice(&compact.block_height.to_le_bytes());
    data.extend_from_slice(&compact.nonce.to_le_bytes());
    for short_id in &compact.short_txids {
        data.extend_from_slice(short_id);
    }
    data
}

fn extract_block_hash(data: &[u8]) -> Hash {
    if data.len() >= 32 {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&data[..32]);
        hash
    } else {
        [0u8; 32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock implementations for testing
    struct MockNetwork;
    struct MockConsensus;
    struct MockMempool;
    struct MockSigVerifier;

    impl PeerNetwork for MockNetwork {
        fn get_connected_peers(&self) -> Vec<PeerInfo> {
            vec![
                PeerInfo {
                    peer_id: PeerId::new([1u8; 32]),
                    reputation: 0.9,
                    latency_ms: 50,
                    is_connected: true,
                },
                PeerInfo {
                    peer_id: PeerId::new([2u8; 32]),
                    reputation: 0.8,
                    latency_ms: 100,
                    is_connected: true,
                },
            ]
        }

        fn send_to_peer(
            &self,
            _peer_id: PeerId,
            _message: NetworkMessage,
        ) -> Result<(), PropagationError> {
            Ok(())
        }

        fn broadcast(
            &self,
            peer_ids: &[PeerId],
            _message: NetworkMessage,
        ) -> Vec<Result<(), PropagationError>> {
            peer_ids.iter().map(|_| Ok(())).collect()
        }
    }

    impl ConsensusGateway for MockConsensus {
        fn submit_block_for_validation(
            &self,
            _block_hash: Hash,
            _block_data: Vec<u8>,
            _source_peer: PeerId,
        ) -> Result<(), PropagationError> {
            Ok(())
        }
    }

    impl MempoolGateway for MockMempool {
        fn get_transactions_by_short_ids(
            &self,
            _short_ids: &[crate::domain::ShortTxId],
            _nonce: u64,
        ) -> Vec<Option<Hash>> {
            Vec::new()
        }
    }

    impl SignatureVerifier for MockSigVerifier {
        fn verify_block_signature(
            &self,
            _block_hash: &Hash,
            _proposer_pubkey: &[u8],
            _signature: &[u8],
        ) -> Result<bool, PropagationError> {
            Ok(true)
        }
    }

    fn create_test_service(
    ) -> BlockPropagationService<MockNetwork, MockConsensus, MockMempool, MockSigVerifier> {
        BlockPropagationService::new(
            PropagationConfig::default(),
            Arc::new(MockNetwork),
            Arc::new(MockConsensus),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
        )
    }

    #[test]
    fn test_propagate_block() {
        let service = create_test_service();
        let block_hash = [0xABu8; 32];
        let block_data = vec![0u8; 1000];
        let tx_hashes = vec![[1u8; 32], [2u8; 32]];

        let result = service.propagate_block(block_hash, block_data, tx_hashes);
        assert!(result.is_ok());

        let stats = result.unwrap();
        assert_eq!(stats.block_hash, block_hash);
        assert!(stats.peers_reached > 0);
    }

    #[test]
    fn test_reject_duplicate_block() {
        let service = create_test_service();
        let block_hash = [0xABu8; 32];
        let block_data = vec![0u8; 1000];
        let tx_hashes = vec![[1u8; 32]];

        // First propagation should succeed
        let result1 = service.propagate_block(block_hash, block_data.clone(), tx_hashes.clone());
        assert!(result1.is_ok());

        // Second propagation should fail
        let result2 = service.propagate_block(block_hash, block_data, tx_hashes);
        assert!(matches!(result2, Err(PropagationError::DuplicateBlock(_))));
    }

    #[test]
    fn test_reject_oversized_block() {
        let config = PropagationConfig {
            max_block_size_bytes: 1000,
            ..Default::default()
        };
        let service = BlockPropagationService::new(
            config,
            Arc::new(MockNetwork),
            Arc::new(MockConsensus),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
        );

        let block_hash = [0xABu8; 32];
        let block_data = vec![0u8; 2000]; // Too large
        let tx_hashes = vec![[1u8; 32]];

        let result = service.propagate_block(block_hash, block_data, tx_hashes);
        assert!(matches!(
            result,
            Err(PropagationError::BlockTooLarge { .. })
        ));
    }

    #[test]
    fn test_get_propagation_status() {
        let service = create_test_service();
        let block_hash = [0xABu8; 32];

        // Initially no status
        let status = service.get_propagation_status(block_hash).unwrap();
        assert!(status.is_none());

        // After propagation
        let block_data = vec![0u8; 1000];
        let tx_hashes = vec![[1u8; 32]];
        let _ = service.propagate_block(block_hash, block_data, tx_hashes);

        let status = service.get_propagation_status(block_hash).unwrap();
        assert!(matches!(status, Some(PropagationState::Complete)));
    }
}
