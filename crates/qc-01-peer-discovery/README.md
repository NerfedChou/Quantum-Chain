# Subsystem 01: Peer Discovery & Routing

**Crate:** `qc-01-peer-discovery`  
**Specification:** `SPECS/SPEC-01-PEER-DISCOVERY.md` v2.4  
**Architecture Compliance:** Architecture.md v2.2 (Hexagonal, DDD, EDA)

## Overview

This crate implements the Kademlia Distributed Hash Table (DHT) for peer discovery
and routing in the Quantum-Chain network. It provides:

- **Peer Discovery:** Find and connect to peers in the network
- **Routing Table:** Efficient XOR-based peer organization
- **Security:** DDoS defense, Sybil resistance, Eclipse attack prevention

## Architecture

The crate follows **Hexagonal Architecture** with strict layer separation:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Adapters Layer                           │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │ Event Publisher  │  │ Event Subscriber │  (shared-bus)      │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        IPC Layer                                │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │     Payloads     │  │     Security     │  (per IPC-MATRIX)  │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        Service Layer                            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              PeerDiscoveryService                         │  │
│  │              (implements PeerDiscoveryApi)                │  │
│  └──────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                        Ports Layer                              │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │ PeerDiscoveryApi │  │  NetworkSocket   │  (traits)          │
│  │ (Driving Port)   │  │  TimeSource      │  (Driven Ports)    │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        Domain Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  RoutingTable│  │   KBucket    │  │   NodeId     │  (pure) │
│  │  BannedPeers │  │ PendingPeer  │  │   PeerInfo   │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

## Security Features

### 1. DDoS Edge Defense (INVARIANT-7, INVARIANT-8)

New peers are staged in `pending_verification` until Subsystem 10 confirms their
identity. This prevents unverified peers from polluting the routing table.

```rust
// New peer enters staging area
table.stage_peer(peer, now)?;

// After verification from Subsystem 10
table.on_verification_result(&node_id, true, now)?;
```

### 2. Memory Bomb Defense (INVARIANT-9 - V2.3)

The staging area is bounded by `max_pending_peers` (default: 1024). When full,
incoming requests are immediately dropped (Tail Drop Strategy).

```rust
let config = KademliaConfig {
    max_pending_peers: 1024,  // ~128KB max memory
    ..Default::default()
};
```

### 3. Eclipse Attack Defense (INVARIANT-10 - V2.4)

When a bucket is full and a new verified peer wants to join, we challenge the
oldest peer with a PING. Only if the oldest peer fails to respond do we evict.

```rust
// When bucket is full, oldest peer is challenged
// If alive: new peer rejected, oldest moved to front
// If dead: oldest evicted, new peer inserted
```

### 4. IP Diversity (INVARIANT-3)

Maximum `max_peers_per_subnet` (default: 2) peers per /24 subnet to prevent
Sybil attacks from a single IP range.

## IPC Integration

### Allowed Senders (per IPC-MATRIX.md)

| Request Type | Allowed Subsystems |
|--------------|-------------------|
| `PeerListRequest` | 5 (Block Propagation), 7 (Bloom Filters), 13 (Light Clients) |
| `FullNodeListRequest` | 13 (Light Clients) only |

### Events Published

| Event | Trigger |
|-------|---------|
| `PeerConnected` | Peer successfully added to routing table |
| `PeerDisconnected` | Peer removed from routing table |
| `PeerBanned` | Peer banned for malformed messages |
| `BootstrapCompleted` | Initial network bootstrap finished |
| `RoutingTableWarning` | Health issues detected |

## Usage

### Basic Usage

```rust
use qc_01_peer_discovery::{
    PeerDiscoveryService, PeerDiscoveryApi,
    NodeId, PeerInfo, SocketAddr, IpAddr, Timestamp,
    KademliaConfig,
};

// Create service with custom time source
let local_id = NodeId::new([0u8; 32]);
let config = KademliaConfig::default();
let time_source = Box::new(SystemTimeSource::new());
let mut service = PeerDiscoveryService::new(local_id, config, time_source);

// Stage a new peer for verification
let peer = PeerInfo::new(
    NodeId::new([1u8; 32]),
    SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
    Timestamp::new(1000),
);
service.add_peer(peer.clone())?;

// After Subsystem 10 verifies the peer
service.on_verification_result(&peer.node_id, true)?;

// Find closest peers to a target
let closest = service.find_closest_peers(target_id, 20);

// Get random peers for gossip
let random_peers = service.get_random_peers(8);
```

### IPC Handler Usage

```rust
use qc_01_peer_discovery::{
    IpcHandler, PeerListRequestPayload, AuthorizationRules,
};

let handler = IpcHandler::new();

// Handle incoming PeerListRequest
let response = handler.handle_peer_list_request(
    sender_id,     // From envelope
    timestamp,     // From envelope
    now,
    reply_to,      // From envelope
    &payload,
    &service,
)?;
```

### Event Publishing

```rust
use qc_01_peer_discovery::{
    EventBuilder, PeerDiscoveryEventPublisher, InMemoryEventPublisher,
};

let publisher = InMemoryEventPublisher::new();
let builder = EventBuilder::new();

// Publish peer connected event
let event = builder.peer_connected(peer_info, bucket_index);
publisher.publish(event)?;

// Publish bootstrap completed
let event = builder.bootstrap_completed(50, 5000);
publisher.publish(event)?;
```

## Configuration

```rust
let config = KademliaConfig {
    k: 20,                          // Bucket size
    alpha: 3,                       // Lookup parallelism
    max_peers_per_subnet: 2,        // IP diversity limit
    max_pending_peers: 1024,        // Memory bound
    verification_timeout_secs: 10,  // Pending peer timeout
    eviction_challenge_timeout_secs: 5,  // Challenge timeout
};
```

## Testing

Run all tests:

```bash
cargo test -p qc-01-peer-discovery
```

Run with verbose output:

```bash
cargo test -p qc-01-peer-discovery -- --nocapture
```

## Test Coverage

| Test Group | Description | Count |
|------------|-------------|-------|
| XOR Distance | Symmetric, correct bucket selection | 4 |
| K-Bucket | Size limits, eviction, challenges | 10 |
| IP Diversity | Subnet enforcement | 3 |
| Peer Lifecycle | Find, add, remove, ban | 6 |
| Staging Area | Bounded staging, Tail Drop | 6 |
| Eclipse Defense | Challenge flow, table poisoning | 7 |
| IPC Security | Authorization, validation | 15+ |

## Dependencies

The domain layer has **zero external dependencies** - pure Rust only.

```toml
[dependencies]
# No dependencies - pure domain logic

[dev-dependencies]
# Test utilities added as needed
```

## Related Documentation

- `SPECS/SPEC-01-PEER-DISCOVERY.md` - Full specification
- `Documentation/Architecture.md` - System architecture
- `Documentation/IPC-MATRIX.md` - IPC authorization matrix
- `Documentation/System.md` - Overall system design

## License

See repository LICENSE file.
