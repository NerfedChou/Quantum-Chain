# SPECIFICATION: BLOCK PROPAGATION

**Version:** 2.3  
**Subsystem ID:** 5  
**Bounded Context:** Network & Gossip  
**Crate Name:** `crates/block-propagation`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.3, IPC-MATRIX.md v2.3, System.md v2.3

---

## 1. ABSTRACT

### 1.1 Purpose

The **Block Propagation** subsystem distributes validated blocks across the peer-to-peer network using an epidemic gossip protocol. It ensures new blocks reach all network nodes rapidly while minimizing bandwidth usage through compact block relay techniques.

### 1.2 Responsibility Boundaries

**In Scope:**
- Receive validated blocks from Consensus for propagation
- Broadcast blocks to connected peers using gossip protocol
- Receive blocks from network peers
- Implement compact block relay (BIP152-style)
- Deduplicate incoming block announcements
- Forward received blocks to Consensus for validation
- Manage block announcement rate limiting

**Out of Scope:**
- Block validation (Subsystem 8)
- Peer discovery and connection management (Subsystem 1)
- Block storage (Subsystem 2)
- Transaction propagation (separate subsystem)
- Consensus logic

### 1.3 Key Design Principles

1. **Speed Over Bandwidth:** Prioritize low-latency block propagation; use compact blocks to reduce bandwidth.
2. **Deduplication:** Never process the same block twice; maintain seen-block cache.
3. **Rate Limiting:** Prevent network flooding via per-peer rate limits.
4. **Resilience:** Multiple propagation paths ensure blocks reach all nodes.

### 1.4 Trust Model

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRUST BOUNDARY (V2.3)                        │
├─────────────────────────────────────────────────────────────────┤
│  TRUSTED INPUTS:                                                │
│  ├─ ValidatedBlock from Subsystem 8 (Consensus)                 │
│  └─ Peer list from Subsystem 1 (Peer Discovery)                 │
│                                                                 │
│  UNTRUSTED (requires validation):                               │
│  ├─ Blocks received from network peers                          │
│  └─ Block announcements from peers                              │
│                                                                 │
│  SECURITY (Envelope-Only Identity - V2.2 Amendment):            │
│  For internal IPC, identity from AuthenticatedMessage only.     │
│  For external P2P, peer identity from connection handshake.     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. DOMAIN MODEL

### 2.1 Core Entities

```rust
/// Block announcement (header-first propagation)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockAnnouncement {
    pub block_hash: Hash,
    pub block_height: u64,
    pub parent_hash: Hash,
    pub timestamp: u64,
    pub difficulty: U256,
}

/// Compact block for bandwidth-efficient relay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompactBlock {
    pub header: BlockHeader,
    /// Short transaction IDs (first 6 bytes of tx hash XOR'd with salt)
    pub short_txids: Vec<ShortTxId>,
    /// Prefilled transactions (coinbase + any the sender thinks receiver lacks)
    pub prefilled_txs: Vec<PrefilledTx>,
    /// Salt for short ID calculation (random per block)
    pub nonce: u64,
}

/// Short transaction ID (6 bytes)
pub type ShortTxId = [u8; 6];

/// Prefilled transaction with index
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrefilledTx {
    pub index: u16,
    pub transaction: Transaction,
}

/// Full block request (when compact block reconstruction fails)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockRequest {
    pub block_hash: Hash,
    pub request_id: u64,
}

/// Block response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockResponse {
    pub request_id: u64,
    pub block: Option<Block>,
}

/// Missing transactions request (for compact block reconstruction)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetBlockTxnRequest {
    pub block_hash: Hash,
    pub indices: Vec<u16>,
}

/// Missing transactions response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockTxnResponse {
    pub block_hash: Hash,
    pub transactions: Vec<Transaction>,
}
```

### 2.2 Gossip State

```rust
/// Per-peer propagation state
pub struct PeerPropagationState {
    pub peer_id: PeerId,
    /// Blocks announced by this peer
    pub announced_blocks: LruCache<Hash, Instant>,
    /// Blocks we've sent to this peer
    pub sent_blocks: LruCache<Hash, Instant>,
    /// Last announcement timestamp (for rate limiting)
    pub last_announcement: Instant,
    /// Announcement count in current window
    pub announcement_count: u32,
    /// Peer latency estimate
    pub latency_ms: u64,
    /// Reputation score (higher = faster/more reliable)
    pub reputation: f64,
}

/// Block propagation configuration
#[derive(Clone, Debug)]
pub struct PropagationConfig {
    /// Number of peers to gossip to (fan-out)
    pub fanout: usize,
    /// Maximum announcements per peer per second
    pub max_announcements_per_second: u32,
    /// Maximum block size
    pub max_block_size_bytes: usize,
    /// Seen block cache size
    pub seen_cache_size: usize,
    /// Compact block reconstruction timeout
    pub reconstruction_timeout_ms: u64,
    /// Full block request timeout
    pub request_timeout_ms: u64,
}

impl Default for PropagationConfig {
    fn default() -> Self {
        Self {
            fanout: 8,
            max_announcements_per_second: 1,
            max_block_size_bytes: 10 * 1024 * 1024,  // 10 MB
            seen_cache_size: 10_000,
            reconstruction_timeout_ms: 5_000,
            request_timeout_ms: 10_000,
        }
    }
}

/// Seen block cache (for deduplication)
pub struct SeenBlockCache {
    seen: LruCache<Hash, SeenBlockInfo>,
}

/// Information about a seen block
pub struct SeenBlockInfo {
    pub first_seen: Instant,
    pub first_peer: Option<PeerId>,
    pub propagation_state: PropagationState,
}

#[derive(Clone, Copy, Debug)]
pub enum PropagationState {
    Announced,        // Header received
    CompactReceived,  // Compact block received
    Reconstructing,   // Waiting for missing txs
    Complete,         // Full block available
    Validated,        // Consensus validated
    Invalid,          // Failed validation
}
```

### 2.3 Invariants

```rust
/// INVARIANT-1: Deduplication
/// The same block hash is never processed/validated twice.
fn invariant_no_duplicate_processing(cache: &SeenBlockCache, hash: &Hash) -> bool {
    if let Some(info) = cache.get(hash) {
        // If we've already fully processed, don't process again
        !matches!(info.propagation_state, PropagationState::Complete | PropagationState::Validated)
    } else {
        true
    }
}

/// INVARIANT-2: Rate Limiting
/// No peer can send more than max_announcements_per_second.
fn invariant_rate_limit(peer: &PeerPropagationState, config: &PropagationConfig) -> bool {
    peer.announcement_count <= config.max_announcements_per_second
}

/// INVARIANT-3: Size Limit
/// No block larger than max_block_size is accepted.
fn invariant_size_limit(block: &Block, config: &PropagationConfig) -> bool {
    block.encoded_size() <= config.max_block_size_bytes
}
```

---

## 3. PORTS (HEXAGONAL ARCHITECTURE)

### 3.1 Driving Ports (API - Inbound)

```rust
/// Primary API for block propagation
#[async_trait]
pub trait BlockPropagationApi: Send + Sync {
    /// Propagate a validated block to the network
    /// Called by Consensus after block validation
    async fn propagate_block(
        &self,
        block: ValidatedBlock,
        consensus_proof: ConsensusProof,
    ) -> Result<PropagationStats, PropagationError>;
    
    /// Get propagation status for a block
    async fn get_propagation_status(
        &self,
        block_hash: Hash,
    ) -> Result<Option<PropagationState>, PropagationError>;
    
    /// Get network propagation metrics
    async fn get_propagation_metrics(&self) -> PropagationMetrics;
}

/// Propagation statistics
#[derive(Clone, Debug)]
pub struct PropagationStats {
    pub block_hash: Hash,
    pub peers_reached: usize,
    pub propagation_start: Instant,
    pub first_ack_time_ms: Option<u64>,
}

/// Network propagation metrics
#[derive(Clone, Debug)]
pub struct PropagationMetrics {
    pub average_propagation_time_ms: f64,
    pub blocks_propagated_last_hour: u64,
    pub compact_block_success_rate: f64,
    pub average_missing_txs: f64,
}
```

### 3.2 Driven Ports (SPI - Outbound Dependencies)

```rust
/// Peer network interface
#[async_trait]
pub trait PeerNetwork: Send + Sync {
    /// Get list of connected peers
    async fn get_connected_peers(&self) -> Vec<PeerInfo>;
    
    /// Send message to a specific peer
    async fn send_to_peer(
        &self,
        peer_id: PeerId,
        message: NetworkMessage,
    ) -> Result<(), NetworkError>;
    
    /// Broadcast to multiple peers
    async fn broadcast(
        &self,
        peer_ids: &[PeerId],
        message: NetworkMessage,
    ) -> Vec<Result<(), NetworkError>>;
    
    /// Subscribe to incoming messages
    async fn subscribe(&self) -> Receiver<(PeerId, NetworkMessage)>;
}

/// Consensus interface for block validation
#[async_trait]
pub trait ConsensusGateway: Send + Sync {
    /// Submit a received block for validation
    async fn submit_block_for_validation(
        &self,
        block: Block,
        source_peer: PeerId,
    ) -> Result<(), ConsensusError>;
}

/// Mempool interface for compact block reconstruction
#[async_trait]
pub trait MempoolGateway: Send + Sync {
    /// Get transactions by short IDs for compact block reconstruction
    async fn get_transactions_by_short_ids(
        &self,
        short_ids: &[ShortTxId],
        nonce: u64,
    ) -> Vec<Option<Transaction>>;
}

/// Signature verification gateway
/// 
/// Reference: IPC-MATRIX.md, Subsystem 10 - "Subsystem 5 (Block Propagation)"
///            listed in "Who Is Allowed To Talk To Me"
#[async_trait]
pub trait SignatureVerifier: Send + Sync {
    /// Verify block proposer signature
    /// 
    /// Security Note: Invalid signatures result in SILENT DROP, not ban.
    /// Reference: Architecture.md - IP spoofing defense
    async fn verify_block_signature(
        &self,
        block_hash: &Hash,
        proposer: &ValidatorId,
        signature: &Signature,
    ) -> Result<bool, VerificationError>;
}
```

---

## 4. EVENT SCHEMA

### 4.1 Internal IPC Messages

```rust
/// Request to propagate a validated block
/// SECURITY: Envelope sender_id MUST be 8 (Consensus)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PropagateBlockRequest {
    pub block: ValidatedBlock,
    pub consensus_proof: ConsensusProof,
}

/// Block received from network (forwarded to Consensus)
/// SECURITY: This is an internal notification, not external
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockReceivedNotification {
    pub block_hash: Hash,
    pub block: Block,
    pub source_peer: PeerId,
    pub received_at: u64,
}

/// Peer list request to Subsystem 1
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GetPeersRequest {
    pub correlation_id: CorrelationId,
    pub reply_to: Topic,
    pub min_reputation: Option<f64>,
    pub max_count: usize,
}
```

### 4.2 P2P Network Messages

```rust
/// Network message types for block propagation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockPropagationMessage {
    /// Block announcement (header-first)
    Announce(BlockAnnouncement),
    /// Compact block
    CompactBlock(CompactBlock),
    /// Full block request
    GetBlock(BlockRequest),
    /// Full block response
    Block(BlockResponse),
    /// Request missing transactions for compact block
    GetBlockTxn(GetBlockTxnRequest),
    /// Missing transactions response
    BlockTxn(BlockTxnResponse),
}
```

### 4.3 Message Flow: Block Propagation

```
OUTBOUND (Local block to network):
┌─────────────────────────────────────────────────────────────────────────────┐
│  [Consensus (8)] ──PropagateBlockRequest──→ [Block Propagation (5)]         │
│                                                    │                         │
│                                                    ↓ select peers (fanout)   │
│                                            ┌───────┴───────┐                 │
│                                            ↓               ↓                 │
│                                       [Peer A]        [Peer B] ...           │
│                                            │               │                 │
│                                   CompactBlock      CompactBlock             │
└─────────────────────────────────────────────────────────────────────────────┘

INBOUND (Network block to local) - WITH SIGNATURE VERIFICATION:
┌─────────────────────────────────────────────────────────────────────────────┐
│  [Peer A] ──CompactBlock──→ [Block Propagation (5)]                         │
│                                      │                                       │
│                                      ↓ reconstruct from mempool              │
│                              ┌───────┴───────┐                               │
│                      [Success]               [Missing TXs]                   │
│                          │                        │                          │
│                          ↓                        ↓                          │
│              [SIGNATURE VERIFICATION]     GetBlockTxn → [Peer A]             │
│                          │                        │                          │
│                          ↓ VerifyBlockSignatureRequest                       │
│                   [Subsystem 10]          BlockTxn received                  │
│                          │                        │                          │
│         ┌────────────────┴────────────────┐       │                          │
│         ↓                                 ↓       │                          │
│    [Valid Sig]                    [Invalid Sig]   │                          │
│         │                               │         │                          │
│         ↓                               ↓         │                          │
│  BlockReceivedNotification     SILENT DROP + BAN  │                          │
│         │                     (per IPC-MATRIX.md) │                          │
│         ↓                                         │                          │
│   [Consensus (8)]                                 │                          │
│                                                   ↓                          │
│                                     reconstruct + verify sig                 │
│                                                   │                          │
│                                                   ↓                          │
│                                      BlockReceivedNotification               │
│                                                   │                          │
│                                                   ↓                          │
│                                             [Consensus (8)]                  │
└─────────────────────────────────────────────────────────────────────────────┘

SECURITY NOTE (IPC-MATRIX.md, Subsystem 10):
- Block Propagation (5) is explicitly authorized to query Signature Verification (10)
- Invalid block signatures result in SILENT DROP, not BAN (IP spoofing defense)
- Reference: Architecture.md §3.2.1 - Envelope-Only Identity
```

---

## 5. TEST-DRIVEN DEVELOPMENT STRATEGY

### 5.1 Unit Tests

```rust
#[cfg(test)]
mod unit_tests {
    use super::*;
    
    #[test]
    fn test_compact_block_short_id_calculation() {
        let tx_hash = [0xAB; 32];
        let nonce = 12345u64;
        
        let short_id = calculate_short_id(&tx_hash, nonce);
        
        // Short ID should be 6 bytes
        assert_eq!(short_id.len(), 6);
        
        // Same inputs should produce same output
        let short_id2 = calculate_short_id(&tx_hash, nonce);
        assert_eq!(short_id, short_id2);
        
        // Different nonce should produce different output
        let short_id3 = calculate_short_id(&tx_hash, nonce + 1);
        assert_ne!(short_id, short_id3);
    }
    
    #[test]
    fn test_compact_block_reconstruction_success() {
        // Setup: Create a block with transactions
        let txs = vec![
            create_test_transaction(1),
            create_test_transaction(2),
            create_test_transaction(3),
        ];
        let block = create_test_block(&txs);
        let compact = create_compact_block(&block, 42);
        
        // All transactions in "mempool"
        let mempool_txs: HashMap<ShortTxId, Transaction> = txs.iter()
            .map(|tx| (calculate_short_id(&tx.hash(), 42), tx.clone()))
            .collect();
        
        // Reconstruct
        let result = reconstruct_block(&compact, |ids, _| {
            ids.iter().map(|id| mempool_txs.get(id).cloned()).collect()
        });
        
        assert!(result.is_ok());
        let reconstructed = result.unwrap();
        assert_eq!(reconstructed.hash(), block.hash());
    }
    
    #[test]
    fn test_compact_block_reconstruction_missing_tx() {
        let txs = vec![
            create_test_transaction(1),
            create_test_transaction(2),
            create_test_transaction(3),
        ];
        let block = create_test_block(&txs);
        let compact = create_compact_block(&block, 42);
        
        // Missing one transaction
        let mempool_txs: HashMap<ShortTxId, Transaction> = txs[..2].iter()
            .map(|tx| (calculate_short_id(&tx.hash(), 42), tx.clone()))
            .collect();
        
        let result = reconstruct_block(&compact, |ids, _| {
            ids.iter().map(|id| mempool_txs.get(id).cloned()).collect()
        });
        
        assert!(matches!(result, Err(ReconstructionError::MissingTransactions(indices)) if indices == vec![2]));
    }
    
    #[test]
    fn test_rate_limiting() {
        let config = PropagationConfig {
            max_announcements_per_second: 1,
            ..Default::default()
        };
        
        let mut peer_state = PeerPropagationState::new(PeerId::random());
        
        // First announcement should be allowed
        assert!(check_rate_limit(&peer_state, &config));
        peer_state.record_announcement();
        
        // Second announcement in same second should be rejected
        assert!(!check_rate_limit(&peer_state, &config));
        
        // After reset, should be allowed again
        peer_state.reset_rate_limit();
        assert!(check_rate_limit(&peer_state, &config));
    }
    
    #[test]
    fn test_deduplication() {
        let mut cache = SeenBlockCache::new(1000);
        let block_hash = [0xAB; 32];
        
        // First time seeing block
        assert!(!cache.has_seen(&block_hash));
        
        // Mark as seen
        cache.mark_seen(block_hash, None);
        
        // Now should be seen
        assert!(cache.has_seen(&block_hash));
    }
    
    #[test]
    fn test_peer_selection_prioritizes_reputation() {
        let peers = vec![
            create_peer_state(1, 0.5),  // Low reputation
            create_peer_state(2, 0.9),  // High reputation
            create_peer_state(3, 0.7),  // Medium reputation
        ];
        
        let selected = select_peers_for_propagation(&peers, 2);
        
        // Should select highest reputation peers first
        assert_eq!(selected.len(), 2);
        assert!(selected.iter().any(|p| p.reputation == 0.9));
        assert!(selected.iter().any(|p| p.reputation == 0.7));
    }
}
```

### 5.2 Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_propagate_block_to_network() {
        // Setup
        let (network, peer_rx) = create_mock_network(3);  // 3 mock peers
        let config = PropagationConfig { fanout: 2, ..Default::default() };
        let service = BlockPropagationService::new(network, config);
        
        // Create validated block
        let block = create_test_validated_block();
        let proof = ConsensusProof::mock();
        
        // Propagate
        let stats = service.propagate_block(block.clone(), proof).await.unwrap();
        
        // Verify: fanout peers received compact blocks
        assert_eq!(stats.peers_reached, 2);
        
        let mut received_count = 0;
        while let Ok((peer_id, msg)) = peer_rx.try_recv() {
            if let NetworkMessage::BlockPropagation(BlockPropagationMessage::CompactBlock(cb)) = msg {
                assert_eq!(cb.header.hash(), block.hash());
                received_count += 1;
            }
        }
        assert_eq!(received_count, 2);
    }
    
    #[tokio::test]
    async fn test_receive_block_from_network() {
        // Setup
        let (network, _) = create_mock_network(1);
        let (consensus, consensus_rx) = create_mock_consensus();
        let service = BlockPropagationService::new(network, consensus);
        
        // Simulate receiving a block from peer
        let block = create_test_block(&[]);
        let peer_id = PeerId::random();
        
        service.handle_incoming_block(peer_id, block.clone()).await.unwrap();
        
        // Verify: Block forwarded to Consensus
        let notification = consensus_rx.recv().await.unwrap();
        assert_eq!(notification.block_hash, block.hash());
        assert_eq!(notification.source_peer, peer_id);
    }
    
    #[tokio::test]
    async fn test_compact_block_reconstruction_with_mempool() {
        // Setup
        let (network, _) = create_mock_network(1);
        let (mempool, _) = create_mock_mempool_with_txs(vec![
            create_test_transaction(1),
            create_test_transaction(2),
        ]);
        let service = BlockPropagationService::new(network, mempool);
        
        // Create compact block
        let txs = vec![
            create_test_transaction(1),
            create_test_transaction(2),
        ];
        let block = create_test_block(&txs);
        let compact = create_compact_block(&block, 42);
        
        // Receive compact block
        let result = service.handle_compact_block(PeerId::random(), compact).await;
        
        // Should successfully reconstruct
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_reject_duplicate_block() {
        let service = create_test_service();
        let block = create_test_block(&[]);
        let peer = PeerId::random();
        
        // First reception should succeed
        let result1 = service.handle_incoming_block(peer, block.clone()).await;
        assert!(result1.is_ok());
        
        // Second reception should be silently ignored (deduplicated)
        let result2 = service.handle_incoming_block(peer, block.clone()).await;
        assert!(matches!(result2, Err(PropagationError::DuplicateBlock)));
    }
    
    #[tokio::test]
    async fn test_reject_rate_limited_peer() {
        let config = PropagationConfig {
            max_announcements_per_second: 1,
            ..Default::default()
        };
        let service = create_test_service_with_config(config);
        let peer = PeerId::random();
        
        // First announcement should succeed
        let ann1 = create_block_announcement(1);
        let result1 = service.handle_announcement(peer, ann1).await;
        assert!(result1.is_ok());
        
        // Second announcement in same window should be rejected
        let ann2 = create_block_announcement(2);
        let result2 = service.handle_announcement(peer, ann2).await;
        assert!(matches!(result2, Err(PropagationError::RateLimited)));
    }
}
```

### 5.3 Security Tests

```rust
#[cfg(test)]
mod security_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_reject_oversized_block() {
        let config = PropagationConfig {
            max_block_size_bytes: 1000,
            ..Default::default()
        };
        let service = create_test_service_with_config(config);
        
        // Create oversized block
        let large_block = create_block_with_size(2000);
        
        let result = service.handle_incoming_block(PeerId::random(), large_block).await;
        assert!(matches!(result, Err(PropagationError::BlockTooLarge)));
    }
    
    #[tokio::test]
    async fn test_reject_block_from_unknown_peer() {
        let service = create_test_service();
        
        // Unknown peer not in connected peers list
        let unknown_peer = PeerId::random();
        let block = create_test_block(&[]);
        
        let result = service.handle_incoming_block(unknown_peer, block).await;
        assert!(matches!(result, Err(PropagationError::UnknownPeer)));
    }
    
    #[test]
    fn test_short_id_collision_resistance() {
        // Ensure short IDs have low collision probability
        let mut short_ids = HashSet::new();
        let nonce = rand::random();
        
        for i in 0..10_000 {
            let tx_hash = [i as u8; 32];
            let short_id = calculate_short_id(&tx_hash, nonce);
            
            // Check for collision
            assert!(short_ids.insert(short_id), "Short ID collision detected!");
        }
    }
    
    #[tokio::test]
    async fn test_propagate_only_from_consensus() {
        let service = create_test_service();
        
        let block = create_test_validated_block();
        let proof = ConsensusProof::mock();
        
        // Create envelope from wrong sender
        let envelope = create_authenticated_message(
            SubsystemId::Mempool,  // Wrong!
            PropagateBlockRequest { block, consensus_proof: proof },
        );
        
        let result = service.handle_propagate_request(envelope).await;
        assert!(matches!(result, Err(PropagationError::UnauthorizedSender)));
    }
}
```

---

## 6. ERROR HANDLING

```rust
#[derive(Debug, thiserror::Error)]
pub enum PropagationError {
    #[error("Block already seen: {0:?}")]
    DuplicateBlock(Hash),
    
    #[error("Block too large: {size} bytes (max: {max})")]
    BlockTooLarge { size: usize, max: usize },
    
    #[error("Peer rate limited: {peer_id:?}")]
    RateLimited { peer_id: PeerId },
    
    #[error("Unknown peer: {0:?}")]
    UnknownPeer(PeerId),
    
    #[error("Compact block reconstruction failed: missing {count} transactions")]
    ReconstructionFailed { count: usize },
    
    #[error("Request timeout: {0:?}")]
    Timeout(Hash),
    
    #[error("Unauthorized sender: {0:?}")]
    UnauthorizedSender(SubsystemId),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] NetworkError),
}
```

---

## 7. CONFIGURATION

```toml
[block_propagation]
# Gossip settings
fanout = 8
max_announcements_per_second = 1

# Size limits
max_block_size_bytes = 10485760  # 10 MB

# Caches
seen_cache_size = 10000

# Timeouts
reconstruction_timeout_ms = 5000
request_timeout_ms = 10000

# Compact blocks
enable_compact_blocks = true
prefill_coinbase = true
```

---

## APPENDIX A: DEPENDENCY MATRIX

**Reference:** System.md V2.3 Unified Dependency Graph, IPC-MATRIX.md Subsystem 5

| This Subsystem | Depends On | Dependency Type | Purpose | Reference |
|----------------|------------|-----------------|---------|-----------|
| Block Prop (5) | Subsystem 1 (Peer Discovery) | Request | Peer list for gossip | System.md Subsystem 5 |
| Block Prop (5) | Subsystem 8 (Consensus) | Accepts from | Validated blocks to propagate | IPC-MATRIX.md Subsystem 5 |
| Block Prop (5) | Subsystem 8 (Consensus) | Sends to | Received blocks for validation | IPC-MATRIX.md Subsystem 5 |
| Block Prop (5) | Subsystem 6 (Mempool) | Query | Transaction lookup for compact blocks | System.md Subsystem 5 |
| Block Prop (5) | Subsystem 10 (Sig Verify) | Query | Block signature verification | IPC-MATRIX.md Subsystem 10 |

---

## APPENDIX B: SECURITY BOUNDARIES (IPC-MATRIX.md Compliance)

**Reference:** IPC-MATRIX.md, Subsystem 5 Section

### B.1 Authorized Message Senders

| Message Type | Authorized Sender(s) | Reference |
|--------------|---------------------|-----------|
| `PropagateBlockRequest` | Subsystem 8 (Consensus) ONLY | IPC-MATRIX.md Security Boundaries |
| `BlockReceived` | External network peers (via P2P) | IPC-MATRIX.md External Sources |
| `BlockRequestReceived` | External network peers | IPC-MATRIX.md External Sources |

### B.2 Mandatory Rejection Rules

**Reference:** IPC-MATRIX.md, Subsystem 5 Security Boundaries

```rust
/// MANDATORY security checks per IPC-MATRIX.md
fn validate_block_propagation_request(
    msg: &AuthenticatedMessage<PropagateBlockRequest>
) -> Result<(), PropagationError> {
    // Rule 1: Only Consensus can request propagation
    if msg.sender_id != SubsystemId::Consensus {
        return Err(PropagationError::UnauthorizedSender(msg.sender_id));
    }
    
    // Rule 2: Block must have ConsensusProof
    if msg.payload.consensus_proof.is_none() {
        return Err(PropagationError::MissingConsensusProof);
    }
    
    // Rule 3: Block size limit (10MB)
    if msg.payload.block.encoded_size() > MAX_BLOCK_SIZE {
        return Err(PropagationError::BlockTooLarge);
    }
    
    Ok(())
}

fn validate_network_block(
    block: &Block,
    source_peer: PeerId,
    peer_list: &PeerList,
) -> Result<(), PropagationError> {
    // Rule 1: Only accept from known peers
    if !peer_list.contains(&source_peer) {
        return Err(PropagationError::UnknownPeer(source_peer));
    }
    
    // Rule 2: Rate limit (1 block announcement per peer per second)
    if rate_limited(source_peer) {
        return Err(PropagationError::RateLimited);
    }
    
    // Rule 3: Size limit
    if block.encoded_size() > MAX_BLOCK_SIZE {
        return Err(PropagationError::BlockTooLarge);
    }
    
    Ok(())
}
```

### B.3 Envelope-Only Identity (V2.2 Amendment)

**Reference:** Architecture.md Section 3.2.1

For internal IPC messages:
- Identity is derived SOLELY from `AuthenticatedMessage.sender_id`
- Payloads MUST NOT contain `requester_id` fields

For external P2P messages:
- Identity is derived from peer connection handshake (Subsystem 1)
- All external blocks are untrusted until validated by Consensus (8)

---

## APPENDIX C: CROSS-REFERENCES

### C.1 Master Document References

| Document | Section | Relevance to This Spec |
|----------|---------|------------------------|
| Architecture.md | Section 3.2.1 | Envelope-Only Identity mandate |
| Architecture.md | Section 5.1 | Event flow (Consensus publishes BlockValidated) |
| IPC-MATRIX.md | Subsystem 5 | Security boundaries, message types |
| System.md | Subsystem 5 | Gossip Protocol algorithm, fan-out |
| System.md | V2.3 Dependency Graph | Depends on 1 (Peer list), 8 (Block validation) |

### C.2 Related Specifications

| Specification | Relationship | Notes |
|--------------|--------------|-------|
| SPEC-01-PEER-DISCOVERY.md | Dependency | Provides peer list for gossip |
| SPEC-06-MEMPOOL.md | Dependency | Provides transactions for compact block reconstruction |
| SPEC-08-CONSENSUS.md | Bidirectional | Receives blocks to propagate; sends received blocks for validation |
| SPEC-10-SIGNATURE-VERIFICATION.md | Dependency | Block signature verification |

### C.3 Implementation Priority

**Reference:** System.md, Implementation Priority Section

This subsystem is in **Phase 2 (Consensus - Weeks 5-8)** because:
- Depends on Subsystem 1 (Peer Discovery) for peer lists
- Depends on Subsystem 8 (Consensus) for block validation
- Required for network-wide block distribution

---

## APPENDIX D: COMPACT BLOCK RELAY (BIP152-Style)

**Reference:** System.md, Subsystem 5 Supporting Algorithms

### D.1 Short Transaction ID Calculation

```rust
/// Short ID calculation for compact block relay
/// 
/// Reference: BIP152 (Bitcoin Improvement Proposal 152)
/// 
/// Formula: short_id = SipHash(nonce || tx_hash)[0:6]
/// 
/// The nonce is randomly generated per block to prevent precomputation attacks.
pub fn calculate_short_id(tx_hash: &Hash, nonce: u64) -> ShortTxId {
    let mut hasher = SipHasher::new_with_keys(nonce, 0);
    hasher.write(tx_hash);
    let full_hash = hasher.finish();
    
    // Take first 6 bytes
    let mut short_id = [0u8; 6];
    short_id.copy_from_slice(&full_hash.to_le_bytes()[..6]);
    short_id
}
```

### D.2 Compact Block Reconstruction Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    COMPACT BLOCK RECONSTRUCTION                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Receive CompactBlock from peer                                          │
│     ├── header: BlockHeader                                                 │
│     ├── short_txids: Vec<[u8; 6]>                                          │
│     ├── prefilled_txs: Vec<(index, Transaction)>                           │
│     └── nonce: u64                                                          │
│                                                                             │
│  2. Look up transactions in local Mempool                                   │
│     ├── For each short_txid, query Mempool.get_by_short_id(short_txid, nonce)│
│     └── Mark missing indices                                                │
│                                                                             │
│  3. If all transactions found → Reconstruct block immediately               │
│                                                                             │
│  4. If missing transactions:                                                │
│     ├── Send GetBlockTxn { block_hash, missing_indices } to peer            │
│     ├── Wait for BlockTxn response (timeout: 5s)                           │
│     └── Reconstruct block with received transactions                        │
│                                                                             │
│  5. Forward reconstructed block to Consensus for validation                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)
