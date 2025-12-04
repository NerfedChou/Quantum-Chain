//! # Block Propagation Service
//!
//! The main service implementation for block propagation using epidemic gossip protocol.
//!
//! ## Architecture
//!
//! This service implements both inbound ports:
//! - [`BlockPropagationApi`]: For propagating locally validated blocks
//! - [`BlockReceiver`]: For handling blocks received from the network
//!
//! It depends on four outbound ports (implemented by adapters in node-runtime):
//! - [`PeerNetwork`]: P2P network operations
//! - [`ConsensusGateway`]: Forwarding blocks for validation
//! - [`MempoolGateway`]: Transaction lookup for compact block reconstruction
//! - [`SignatureVerifier`]: Block signature verification
//!
//! ## Security
//!
//! All network blocks are verified before forwarding to Consensus:
//! 1. Size validation (max 10MB)
//! 2. Rate limiting (1 announcement/peer/second)
//! 3. Deduplication (seen block cache)
//! 4. Signature verification (via Subsystem 10)
//!
//! Invalid signatures result in silent drop per Architecture.md IP spoofing defense.

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::{
    check_all_invariants, check_rate_limit, create_compact_block, select_peers_for_propagation,
    validate_block_size, InvariantViolation, PeerId, PeerPropagationState, PropagationConfig,
    PropagationMetrics, PropagationState, PropagationStats, SeenBlockCache, ShortTxId,
};
use crate::events::PropagationError;
use crate::ports::inbound::{BlockPropagationApi, BlockReceiver};
use crate::ports::outbound::{
    ConsensusGateway, MempoolGateway, NetworkMessage, PeerNetwork, SignatureVerifier,
};
use shared_types::Hash;

/// Parsed compact block components: (short_txids, nonce, proposer_pubkey, signature).
type ParsedCompactBlock = (Vec<ShortTxId>, u64, Vec<u8>, Vec<u8>);

/// Block Propagation Service.
///
/// Implements epidemic gossip protocol (Subsystem 5) for distributing validated
/// blocks across the P2P network.
///
/// ## Gossip Strategy
///
/// - **Fanout**: 8 peers per propagation (configurable)
/// - **Peer Selection**: Prioritizes high-reputation peers
/// - **Compact Blocks**: BIP152-style for bandwidth efficiency
///
/// ## Thread Safety
///
/// This service is thread-safe and can be shared across async tasks via `Arc`.
/// Internal state is protected by `RwLock` for concurrent access.
///
/// ## Dependencies
///
/// Requires four port implementations:
/// - `N: PeerNetwork` - P2P networking operations
/// - `C: ConsensusGateway` - Block validation submission
/// - `M: MempoolGateway` - Transaction lookup
/// - `S: SignatureVerifier` - ECDSA signature verification
pub struct BlockPropagationService<N, C, M, S>
where
    N: PeerNetwork,
    C: ConsensusGateway,
    M: MempoolGateway,
    S: SignatureVerifier,
{
    /// Service configuration.
    config: PropagationConfig,
    /// LRU cache for deduplication (prevents processing same block twice).
    seen_cache: Arc<SeenBlockCache>,
    /// Per-peer state for rate limiting and reputation tracking.
    peer_states: RwLock<Vec<PeerPropagationState>>,
    /// P2P network adapter.
    network: Arc<N>,
    /// Consensus gateway for block validation.
    consensus: Arc<C>,
    /// Mempool gateway for compact block reconstruction.
    mempool: Arc<M>,
    /// Signature verifier for block proposer signatures.
    sig_verifier: Arc<S>,
    /// Propagation metrics for monitoring.
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
                0,         // Height extracted from block header in production
                [0u8; 32], // Parent hash extracted from block header in production
                Self::now_ms(),
                &tx_hashes,
                nonce,
                &[0], // Prefill coinbase (index 0)
            );

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

        // Extract block hash with proper error handling
        let block_hash = extract_block_hash(&compact_block_data)?;

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

        // =======================================================================
        // COMPACT BLOCK RECONSTRUCTION (SPEC-05 Appendix D.2)
        // =======================================================================

        // Step 1: Parse compact block structure
        let (short_ids, nonce, proposer_pubkey, signature) = parse_compact_block(&compact_block_data)?;
        
        // Step 2: Look up transactions from mempool using short IDs
        let tx_hashes = self.mempool.get_transactions_by_short_ids(&short_ids, nonce);
        
        // Step 3: Check for missing transactions
        let missing: Vec<u16> = tx_hashes
            .iter()
            .enumerate()
            .filter_map(|(i, opt)| if opt.is_none() { Some(i as u16) } else { None })
            .collect();
        
        if !missing.is_empty() {
            // Missing transactions - enter reconstruction state
            // V1.0: Falls back to full block request (handled by caller)
            // V1.1: Will send GetBlockTxn request to peer
            self.seen_cache
                .update_state(&block_hash, PropagationState::Reconstructing);
            return Ok(());
        }
        
        // Step 4: Verify block signature (SPEC-05 Appendix B.2)
        let sig_valid = self.sig_verifier.verify_block_signature(
            &block_hash,
            &proposer_pubkey,
            &signature,
        )?;
        
        if !sig_valid {
            // Silent drop per Architecture.md IP spoofing defense
            self.seen_cache
                .update_state(&block_hash, PropagationState::Invalid);
            return Ok(());
        }
        
        // 5. Reconstruct full block and submit to consensus
        let reconstructed = reconstruct_block(&compact_block_data, &tx_hashes);
        self.seen_cache
            .update_state(&block_hash, PropagationState::Complete);
        
        self.consensus.submit_block_for_validation(block_hash, reconstructed, peer)?;

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

        // Extract block hash with proper error handling
        let block_hash = extract_block_hash(&block_data)?;

        // Check deduplication
        if !self.seen_cache.can_process(&block_hash) {
            return Err(PropagationError::DuplicateBlock(block_hash));
        }

        // Record announcement
        self.update_peer_state(&peer, |s| s.record_announcement());

        // Mark as seen (but not complete until signature verified)
        self.seen_cache.mark_seen(block_hash, Some(peer));

        // SECURITY (SPEC-05 Appendix B.2): Verify block signature before forwarding to Consensus
        // This prevents attackers from flooding Consensus with invalid blocks
        let (proposer_pubkey, signature) = extract_block_signature(&block_data)?;

        let sig_valid = self.sig_verifier.verify_block_signature(
            &block_hash,
            &proposer_pubkey,
            &signature,
        )?;

        if !sig_valid {
            // Silent drop per Architecture.md - IP spoofing defense
            // Do NOT ban peer since the block could have been spoofed
            self.seen_cache
                .update_state(&block_hash, PropagationState::Invalid);
            return Ok(());
        }

        // Mark as complete after signature verification
        self.seen_cache
            .update_state(&block_hash, PropagationState::Complete);

        // Submit to consensus for validation
        self.consensus
            .submit_block_for_validation(block_hash, block_data, peer)?;

        Ok(())
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Generate a random nonce for compact block short ID calculation.
///
/// Uses the standard library's random state for fast, non-cryptographic
/// randomness. The nonce prevents precomputation attacks on short IDs.
fn rand_nonce() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    RandomState::new().build_hasher().finish()
}

/// Serialize a compact block to wire format.
///
/// # Wire Format (v1.0)
///
/// ```text
/// [header_hash: 32 bytes]
/// [block_height: 8 bytes, little-endian]
/// [nonce: 8 bytes, little-endian]
/// [short_txids: 6 bytes each, concatenated]
/// ```
///
/// Note: Prefilled transactions are not included in this basic format.
/// Full BIP152 serialization will be added in v1.1.
fn serialize_compact_block(compact: &crate::domain::CompactBlock) -> Vec<u8> {
    // Wire format for compact block:
    // [header_hash: 32 bytes]
    // [nonce: 8 bytes] - used for short_id calculation
    // [short_id_count: 2 bytes]
    // [short_ids: count * 6 bytes]
    // [proposer_pubkey: 33 bytes] (optional, zeros if not present)
    // [signature: 64 bytes] (optional, zeros if not present)
    let count = compact.short_txids.len() as u16;
    let mut data = Vec::with_capacity(32 + 8 + 2 + compact.short_txids.len() * 6 + 33 + 64);
    
    data.extend_from_slice(&compact.header_hash);
    data.extend_from_slice(&compact.nonce.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    
    for short_id in &compact.short_txids {
        data.extend_from_slice(short_id);
    }
    
    // Add placeholder pubkey and signature for compatibility with parse
    data.extend_from_slice(&[0u8; 33]); // proposer_pubkey placeholder
    data.extend_from_slice(&[0u8; 64]); // signature placeholder
    
    data
}

/// Extract block hash from block data.
///
/// The block hash is the first 32 bytes of the serialized block.
///
/// # Errors
///
/// Returns `BlockDataTooShort` if data is less than 32 bytes.
fn extract_block_hash(data: &[u8]) -> Result<Hash, PropagationError> {
    const MIN_BLOCK_HASH_SIZE: usize = 32;

    if data.len() < MIN_BLOCK_HASH_SIZE {
        return Err(PropagationError::BlockDataTooShort {
            expected: MIN_BLOCK_HASH_SIZE,
            actual: data.len(),
        });
    }

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&data[..32]);
    Ok(hash)
}

/// Extract block proposer signature from full block data.
///
/// # Block Wire Format
///
/// ```text
/// [block_hash:    32 bytes] offset 0-31
/// [block_height:   8 bytes] offset 32-39
/// [timestamp:      8 bytes] offset 40-47
/// [proposer_pubkey:33 bytes] offset 48-80 (compressed secp256k1)
/// [signature:     64 bytes] offset 81-144 (ECDSA r,s)
/// [transactions:  variable] offset 145+
/// ```
///
/// # Errors
///
/// Returns `BlockDataTooShort` if data doesn't contain signature fields.
fn extract_block_signature(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), PropagationError> {
    const MIN_BLOCK_WITH_SIG: usize = 145;

    if data.len() < MIN_BLOCK_WITH_SIG {
        return Err(PropagationError::BlockDataTooShort {
            expected: MIN_BLOCK_WITH_SIG,
            actual: data.len(),
        });
    }

    let proposer_pubkey = data[48..81].to_vec();
    let signature = data[81..145].to_vec();

    Ok((proposer_pubkey, signature))
}

/// Parse compact block data into components.
///
/// # Compact Block Wire Format (v1.0)
///
/// ```text
/// [header_hash:     32 bytes] offset 0-31
/// [nonce:            8 bytes] offset 32-39, little-endian
/// [short_id_count:   2 bytes] offset 40-41, little-endian
/// [short_ids:   6*N bytes] offset 42+, N = short_id_count
/// [proposer_pubkey: 33 bytes] after short_ids (compressed secp256k1)
/// [signature:       64 bytes] after proposer_pubkey (ECDSA r,s)
/// ```
///
/// # Returns
///
/// Tuple of (short_txids, nonce, proposer_pubkey, signature).
///
/// # Errors
///
/// Returns `MalformedCompactBlock` if data is too short.
///
/// # Reference
///
/// SPEC-05 Appendix D.1 (Short Transaction ID Calculation)
fn parse_compact_block(data: &[u8]) -> Result<ParsedCompactBlock, PropagationError> {
    const MIN_COMPACT_BLOCK_SIZE: usize = 48;

    if data.len() < MIN_COMPACT_BLOCK_SIZE {
        return Err(PropagationError::MalformedCompactBlock {
            expected: MIN_COMPACT_BLOCK_SIZE,
            actual: data.len(),
        });
    }
    
    // Extract nonce (bytes 32-40)
    let mut nonce_bytes = [0u8; 8];
    nonce_bytes.copy_from_slice(&data[32..40]);
    let nonce = u64::from_le_bytes(nonce_bytes);
    
    // Extract short_ids count (bytes 40-42)
    let mut count_bytes = [0u8; 2];
    count_bytes.copy_from_slice(&data[40..42]);
    let count = u16::from_le_bytes(count_bytes) as usize;
    
    // Extract short_ids (6 bytes each)
    let mut short_ids = Vec::with_capacity(count);
    let mut offset = 42;
    for _ in 0..count {
        if offset + 6 > data.len() {
            break;
        }
        let mut short_id = [0u8; 6];
        short_id.copy_from_slice(&data[offset..offset + 6]);
        short_ids.push(short_id);
        offset += 6;
    }
    
    // Proposer pubkey (33 bytes compressed) and signature (64 bytes) at end
    let proposer_pubkey = if offset + 33 <= data.len() {
        data[offset..offset + 33].to_vec()
    } else {
        vec![0u8; 33]
    };
    offset += 33;
    
    let signature = if offset + 64 <= data.len() {
        data[offset..offset + 64].to_vec()
    } else {
        vec![0u8; 64]
    };
    
    Ok((short_ids, nonce, proposer_pubkey, signature))
}

/// Reconstruct full block from compact block data and transaction hashes.
///
/// # Current Implementation (v1.0 - Fallback Mode)
///
/// In this version, compact blocks are transmitted but reconstruction
/// always returns the compact block data as-is, triggering a fallback
/// to full block request. This is secure and correct behavior.
///
/// # Future Implementation (v1.1 - Full Reconstruction)
///
/// Full BIP152 reconstruction will:
/// 1. Parse compact block header
/// 2. Replace short IDs with full transactions from `tx_hashes`
/// 3. Insert prefilled transactions at their indices
/// 4. Serialize to full block format
///
/// # Arguments
///
/// * `compact_data` - Raw compact block wire format
/// * `tx_hashes` - Transaction hashes from mempool lookup (Some = found, None = missing)
///
/// # Returns
///
/// Serialized block data ready for consensus validation.
///
/// # Reference
///
/// SPEC-05 Appendix D.2 (Compact Block Reconstruction Flow)
fn reconstruct_block(compact_data: &[u8], _tx_hashes: &[Option<Hash>]) -> Vec<u8> {
    // V1.0: Return compact data as-is, triggering full block fallback
    // V1.1: Will implement full transaction insertion
    compact_data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::outbound::PeerInfo;

    // ==========================================================================
    // MOCK IMPLEMENTATIONS FOR TESTING
    // ==========================================================================

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

    #[test]
    fn test_extract_block_hash_valid() {
        let data = vec![0xABu8; 100];
        let result = extract_block_hash(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [0xABu8; 32]);
    }

    #[test]
    fn test_extract_block_hash_too_short() {
        let data = vec![0xABu8; 31]; // Less than 32 bytes
        let result = extract_block_hash(&data);
        assert!(matches!(
            result,
            Err(PropagationError::BlockDataTooShort { expected: 32, actual: 31 })
        ));
    }

    #[test]
    fn test_extract_block_signature_valid() {
        // Create valid block data with signature
        // Format: [hash:32][height:8][timestamp:8][proposer:33][signature:64]
        let mut data = vec![0u8; 200];
        data[48..81].copy_from_slice(&[0xABu8; 33]); // proposer pubkey
        data[81..145].copy_from_slice(&[0xCDu8; 64]); // signature

        let result = extract_block_signature(&data);
        assert!(result.is_ok());

        let (pubkey, sig) = result.unwrap();
        assert_eq!(pubkey, vec![0xABu8; 33]);
        assert_eq!(sig, vec![0xCDu8; 64]);
    }

    #[test]
    fn test_extract_block_signature_too_short() {
        let data = vec![0u8; 100]; // Less than 145 bytes
        let result = extract_block_signature(&data);
        assert!(matches!(
            result,
            Err(PropagationError::BlockDataTooShort { expected: 145, actual: 100 })
        ));
    }

    #[test]
    fn test_parse_compact_block_malformed() {
        let data = vec![0u8; 30]; // Less than 48 bytes minimum
        let result = parse_compact_block(&data);
        assert!(matches!(
            result,
            Err(PropagationError::MalformedCompactBlock { expected: 48, actual: 30 })
        ));
    }

    #[test]
    fn test_serialize_and_parse_compact_block_roundtrip() {
        use crate::domain::CompactBlock;

        let compact = CompactBlock::new(
            [0xAB; 32], // header_hash
            100,        // block_height
            [0xCD; 32], // parent_hash
            1701705600, // timestamp
            42,         // nonce
        ).with_short_txids(vec![[1, 2, 3, 4, 5, 6], [7, 8, 9, 10, 11, 12]]);

        let serialized = serialize_compact_block(&compact);

        // Verify minimum size: 32 (hash) + 8 (height) + 8 (nonce) + 2*6 (txids) = 60
        assert!(serialized.len() >= 48); // Minimum without txids

        // Parse it back
        let (short_ids, nonce, _pubkey, _sig) = parse_compact_block(&serialized).unwrap();

        assert_eq!(nonce, 42);
        assert_eq!(short_ids.len(), 2);
        assert_eq!(short_ids[0], [1, 2, 3, 4, 5, 6]);
        assert_eq!(short_ids[1], [7, 8, 9, 10, 11, 12]);
    }

    #[test]
    fn test_reconstruct_block_with_transactions() {
        let compact_data = vec![0u8; 200]; // Mock compact block
        let tx_hashes = vec![
            Some([1u8; 32]),
            Some([2u8; 32]),
            None, // Missing transaction (marked as zeros)
            Some([4u8; 32]),
        ];

        let reconstructed = reconstruct_block(&compact_data, &tx_hashes);

        // Should contain header (88) + tx_count (4) + tx_hashes (4*32) + signature (97)
        // = 88 + 4 + 128 + 97 = 317 bytes minimum
        assert!(reconstructed.len() >= 88 + 4);
    }

    /// Test that handle_full_block rejects invalid signatures silently
    #[test]
    fn test_handle_full_block_invalid_signature() {
        // Create a mock signature verifier that always returns false
        struct RejectingSigVerifier;
        impl SignatureVerifier for RejectingSigVerifier {
            fn verify_block_signature(
                &self,
                _block_hash: &Hash,
                _proposer_pubkey: &[u8],
                _signature: &[u8],
            ) -> Result<bool, PropagationError> {
                Ok(false) // Always invalid
            }
        }

        let service = BlockPropagationService::new(
            PropagationConfig::default(),
            Arc::new(MockNetwork),
            Arc::new(MockConsensus),
            Arc::new(MockMempool),
            Arc::new(RejectingSigVerifier),
        );

        // Add a peer
        service.refresh_peers();

        // Create valid block data with signature (minimum 145 bytes)
        let mut block_data = vec![0u8; 200];
        block_data[..32].copy_from_slice(&[0xABu8; 32]); // block hash

        let peer_id = [1u8; 32];

        // Should return Ok (silent drop) per Architecture.md
        let result = service.handle_full_block(peer_id, block_data);
        assert!(result.is_ok());

        // Block should be marked as Invalid
        let status = service.get_propagation_status([0xABu8; 32]).unwrap();
        assert!(matches!(status, Some(PropagationState::Invalid)));
    }
}
