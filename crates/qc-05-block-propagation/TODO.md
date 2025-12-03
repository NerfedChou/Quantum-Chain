# qc-05-block-propagation: Implementation TODO

**Specification:** SPEC-05-BLOCK-PROPAGATION.md  
**Status:** 🔴 NOT STARTED → 🟡 IN PROGRESS  
**Last Updated:** 2024-12-03  

---

## Overview

Block Propagation is a **core subsystem** responsible for distributing validated blocks across the P2P network using epidemic gossip protocol. It implements BIP152-style compact block relay for bandwidth efficiency.

### Architecture Role

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BLOCK PROPAGATION IN V2.3 CHOREOGRAPHY                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  [Consensus (8)] ──PropagateBlockRequest──→ [Block Propagation (5)]         │
│                                                    │                         │
│                                                    ↓ gossip (fanout=8)       │
│                                            ┌───────┴───────┐                 │
│                                            ↓               ↓                 │
│                                       [Peer A]        [Peer B] ...           │
│                                                                             │
│  [Network Peers] ──CompactBlock──→ [Block Propagation (5)]                  │
│                                            │                                │
│                                            ↓ reconstruct + verify sig       │
│                                    [Subsystem 10 (Sig Verify)]              │
│                                            │                                │
│                                            ↓                                │
│                                    [Consensus (8)]                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Dependencies (IPC-MATRIX.md)

| Depends On | Purpose |
|------------|---------|
| Subsystem 1 (Peer Discovery) | Peer list for gossip |
| Subsystem 6 (Mempool) | Transaction lookup for compact block reconstruction |
| Subsystem 8 (Consensus) | Validated blocks to propagate + submit received blocks |
| Subsystem 10 (Sig Verify) | Block signature verification |

---

## Implementation Phases

### Phase 1: RED (Write Failing Tests)
- [ ] Unit tests for short ID calculation (SipHash)
- [ ] Unit tests for compact block creation/reconstruction
- [ ] Unit tests for rate limiting
- [ ] Unit tests for deduplication (seen cache)
- [ ] Unit tests for peer selection (reputation-based)
- [ ] Integration tests for block propagation flow
- [ ] Integration tests for compact block reconstruction with mempool
- [ ] Security tests for oversized block rejection
- [ ] Security tests for rate limiting enforcement
- [ ] Security tests for unauthorized sender rejection

### Phase 2: Domain Layer
- [ ] **Entities:**
  - [ ] `BlockAnnouncement` - header-first propagation
  - [ ] `CompactBlock` - bandwidth-efficient block relay
  - [ ] `ShortTxId` - 6-byte transaction identifier
  - [ ] `PrefilledTx` - prefilled transactions in compact block
  - [ ] `BlockRequest` / `BlockResponse` - full block exchange
  - [ ] `GetBlockTxnRequest` / `BlockTxnResponse` - missing tx request

- [ ] **Value Objects:**
  - [ ] `PropagationConfig` - fanout, rate limits, timeouts
  - [ ] `PeerPropagationState` - per-peer gossip state
  - [ ] `SeenBlockCache` - LRU deduplication cache
  - [ ] `PropagationState` - Announced/CompactReceived/Reconstructing/Complete/Validated/Invalid

- [ ] **Domain Services:**
  - [ ] `calculate_short_id(tx_hash, nonce)` - SipHash-based short ID
  - [ ] `create_compact_block(block, nonce)` - create compact representation
  - [ ] `reconstruct_block(compact, mempool)` - reconstruct from compact + mempool
  - [ ] `select_peers_for_propagation(peers, fanout)` - reputation-based selection

- [ ] **Invariants (SPEC-05 Section 2.3):**
  - [ ] INVARIANT-1: Deduplication - same block hash never processed twice
  - [ ] INVARIANT-2: Rate Limiting - max announcements per peer per second
  - [ ] INVARIANT-3: Size Limit - no block larger than max_block_size

### Phase 3: Ports (Hexagonal Architecture)
- [ ] **Inbound Ports (API):**
  - [ ] `BlockPropagationApi` trait
    - [ ] `propagate_block(ValidatedBlock, ConsensusProof) -> PropagationStats`
    - [ ] `get_propagation_status(Hash) -> Option<PropagationState>`
    - [ ] `get_propagation_metrics() -> PropagationMetrics`

- [ ] **Outbound Ports (SPI):**
  - [ ] `PeerNetwork` trait - peer communication
    - [ ] `get_connected_peers() -> Vec<PeerInfo>`
    - [ ] `send_to_peer(PeerId, NetworkMessage)`
    - [ ] `broadcast(peer_ids, message)`
    - [ ] `subscribe() -> Receiver<(PeerId, NetworkMessage)>`
  - [ ] `ConsensusGateway` trait - submit blocks for validation
  - [ ] `MempoolGateway` trait - transaction lookup for compact blocks
  - [ ] `SignatureVerifier` trait - block signature verification

### Phase 4: Events (EDA)
- [ ] **IPC Messages:**
  - [ ] `PropagateBlockRequest` - from Consensus (sender_id=8 only)
  - [ ] `BlockReceivedNotification` - to Consensus
  - [ ] `GetPeersRequest` - to Peer Discovery

- [ ] **P2P Messages:**
  - [ ] `BlockPropagationMessage` enum
    - [ ] `Announce(BlockAnnouncement)`
    - [ ] `CompactBlock(CompactBlock)`
    - [ ] `GetBlock(BlockRequest)`
    - [ ] `Block(BlockResponse)`
    - [ ] `GetBlockTxn(GetBlockTxnRequest)`
    - [ ] `BlockTxn(BlockTxnResponse)`

### Phase 5: GREEN (Implement to Pass Tests)
- [ ] `BlockPropagationService` - main service implementation
- [ ] Gossip algorithm with fanout
- [ ] Compact block creation and reconstruction
- [ ] Rate limiting per peer
- [ ] Deduplication via seen cache
- [ ] Signature verification integration
- [ ] Error handling with `PropagationError` enum

### Phase 6: IPC Security (Shared Module)
- [ ] Use `shared_types::security::MessageVerifier`
- [ ] HMAC signature validation on all IPC messages
- [ ] Nonce/replay protection
- [ ] Timestamp validation
- [ ] Sender authorization (only Consensus can request propagation)

### Phase 7: Adapters
- [ ] `P2PNetworkAdapter` - libp2p or custom P2P implementation
- [ ] `ConsensusClientAdapter` - IPC to Consensus subsystem
- [ ] `MempoolClientAdapter` - IPC to Mempool subsystem
- [ ] `SignatureVerifierAdapter` - IPC to Signature Verification subsystem

### Phase 8: Integration & Wiring
- [ ] Wire to shared-bus for IPC
- [ ] Register message handlers
- [ ] Integration with node-runtime
- [ ] End-to-end tests with other subsystems

---

## Security Requirements (IPC-MATRIX.md)

### Authorized Senders
| Message Type | Authorized Sender |
|--------------|-------------------|
| `PropagateBlockRequest` | Subsystem 8 (Consensus) ONLY |
| Network blocks | External peers (untrusted, requires validation) |

### Mandatory Checks
1. **IPC Messages:** HMAC signature + nonce + timestamp validation
2. **Network Blocks:** Size limit (10MB), rate limit (1/sec/peer), signature verification
3. **Sender Authorization:** Only Consensus can request block propagation

### Security Invariants
- Invalid block signatures → SILENT DROP (not ban, per IP spoofing defense)
- Unknown peers → reject block
- Oversized blocks → reject
- Rate-limited peers → reject announcements

---

## Performance Targets (SPEC-05)

| Metric | Target |
|--------|--------|
| Fanout | 8 peers |
| Max announcements/peer/sec | 1 |
| Max block size | 10 MB |
| Seen cache size | 10,000 blocks |
| Compact block reconstruction timeout | 5 seconds |
| Full block request timeout | 10 seconds |
| Compact block success rate | >90% (most txs from mempool) |

---

## Brutal Test Coverage

### Attack Vectors to Test
- [ ] Unauthorized propagation request (non-Consensus sender)
- [ ] Oversized block injection
- [ ] Rate limit bypass attempts
- [ ] Replay attacks on IPC messages
- [ ] HMAC signature forgery
- [ ] Short ID collision attacks
- [ ] Compact block reconstruction poisoning
- [ ] Block flooding DoS
- [ ] Invalid block signature injection

---

## Files to Create

```
crates/qc-05-block-propagation/
├── Cargo.toml              # Update dependencies
├── TODO.md                 # This file
├── README.md               # Subsystem documentation
└── src/
    ├── lib.rs              # Module exports
    ├── domain/
    │   ├── mod.rs
    │   ├── entities.rs     # BlockAnnouncement, CompactBlock, etc.
    │   ├── value_objects.rs # Config, state, cache
    │   ├── services.rs     # Short ID, reconstruction, peer selection
    │   └── invariants.rs   # Security invariants
    ├── ports/
    │   ├── mod.rs
    │   ├── inbound.rs      # BlockPropagationApi
    │   └── outbound.rs     # PeerNetwork, ConsensusGateway, etc.
    ├── events/
    │   ├── mod.rs
    │   ├── ipc.rs          # IPC message types
    │   └── p2p.rs          # P2P message types
    ├── service.rs          # BlockPropagationService
    ├── ipc/
    │   ├── mod.rs
    │   ├── handler.rs      # IPC message handlers
    │   └── security.rs     # Uses shared-types security
    └── adapters/           # (Phase 7)
        └── mod.rs
```
