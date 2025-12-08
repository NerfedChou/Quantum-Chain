# Subsystem 01: Peer Discovery & Routing

**Crate:** `qc-01-peer-discovery`  
**Specification:** [`SPECS/SPEC-01-PEER-DISCOVERY.md`](../../SPECS/SPEC-01-PEER-DISCOVERY.md) v2.4  
**Architecture Compliance:** Architecture.md v2.2 (Hexagonal, DDD, EDA)

## Overview

This crate implements the **Kademlia Distributed Hash Table (DHT)** for peer discovery
and routing in the Quantum-Chain network.

> **Scope Boundary (SPEC-01 Section 1.2):**
> - ✅ **In Scope:** Pure domain logic (XOR distance, k-bucket management, invariants)
> - ❌ **Out of Scope:** Network I/O (delegated to `NetworkSocket` adapter, implemented by node-runtime)
>
> This crate is a **library**, not a standalone service. The `node-runtime` provides the
> glue that wires this library to actual network I/O.

### Quick Stats

| Metric | Value |
|--------|-------|
| Lines of Code | ~7,000 |
| Unit Tests | 105 |
| Doc Tests | 4 |
| Files | 24 |

## Architecture

The crate follows **Hexagonal Architecture** with strict layer separation:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Adapters Layer                           │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │ BootstrapHandler │  │  SystemTimeSource│  │UdpNetworkSocket│ │
│  │ EventPublisher   │  │  TomlConfigProvider│  (reference)   │  │
│  └──────────────────┘  └──────────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                        IPC Layer                                │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │  Payloads        │  │  Security        │  │  Handler     │  │
│  │  (per IPC-MATRIX)│  │ (authorization)  │  │              │  │
│  └──────────────────┘  └──────────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                        Service Layer                            │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              PeerDiscoveryService                         │  │
│  │    (implements PeerDiscoveryApi, VerificationHandler)     │  │
│  └──────────────────────────────────────────────────────────┘  │
├─────────────────────────────────────────────────────────────────┤
│                        Ports Layer                              │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │ PeerDiscoveryApi │  │  NetworkSocket   │  (traits)          │
│  │ (Driving Port)   │  │  TimeSource      │  (Driven Ports)    │
│  │                  │  │  ConfigProvider  │                    │
│  │                  │  │  NodeIdValidator │                    │
│  └──────────────────┘  └──────────────────┘                    │
├─────────────────────────────────────────────────────────────────┤
│                        Domain Layer                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  RoutingTable│  │   KBucket    │  │   NodeId     │  (pure) │
│  │  BannedPeers │  │ PendingPeer  │  │   PeerInfo   │         │
│  │  Distance    │  │PendingInsertion│ │   Timestamp  │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

## Security Features

Reference: [System.md lines 49-62, SPEC-01 Section 2.4]

### 1. DDoS Edge Defense (INVARIANT-7, INVARIANT-8)

New peers are staged in `pending_verification` until Subsystem 10 confirms their
identity. This prevents unverified peers from polluting the routing table.

```rust
// New peer enters staging area
service.add_peer(peer)?;  // Stages, does NOT add to routing table

// After verification from Subsystem 10
service.on_verification_result(&node_id, true)?;  // NOW promotes to table
```

### 2. Memory Bomb Defense (INVARIANT-9 - V2.3)

The staging area is bounded by `max_pending_peers` (default: 1024). When full,
incoming requests are immediately dropped (Tail Drop Strategy).

```rust
let config = KademliaConfig {
    max_pending_peers: 1024,  // ~128KB max memory
    ..Default::default()
};

// Returns Err(StagingAreaFull) if staging area is at capacity
let result = service.add_peer(peer);
```

### 3. Eclipse Attack Defense (INVARIANT-10 - V2.4)

When a bucket is full and a new verified peer wants to join, we challenge the
oldest peer with a PING. Only if the oldest peer fails to respond do we evict.

```rust
// Returns Some(node_to_challenge) if a challenge is needed
let challenged = service.on_verification_result(&node_id, true)?;
if let Some(peer_to_ping) = challenged {
    // node-runtime sends PING, then calls:
    service.on_challenge_response(&peer_to_ping, is_alive)?;
}
```

### 4. IP Diversity (INVARIANT-3)

Maximum `max_peers_per_subnet` (default: 2) peers per /24 subnet to prevent
Sybil attacks from a single IP range.

### 5. Silent Drop for Invalid Signatures

Per [SPEC-01 Section 2.2, lines 158-176], `BanReason::InvalidSignature` is
intentionally **NOT** included. Failed signature verification results in
silent drop, not banning, to prevent IP spoofing-based attacks.

## IPC Integration

Reference: [IPC-MATRIX.md lines 8-100]

### Inbound Messages (Handled)

| Message | Source | Handler |
|---------|--------|---------|
| `PeerListRequest` | Subsystems 5, 7, 13 | `IpcHandler::handle_peer_list_request` |
| `FullNodeListRequest` | Subsystem 13 | `IpcHandler::handle_full_node_list_request` |
| `BootstrapRequest` | External nodes | `BootstrapHandler::handle` |
| `NodeIdentityVerificationResult` | Subsystem 10 | `VerificationHandler::handle_verification` |

### Outbound Messages (Published)

| Message | Target | Trait |
|---------|--------|-------|
| `PeerList` | Subsystems 5, 7, 13 | `PeerDiscoveryEventPublisher` |
| `VerifyNodeIdentityRequest` | Subsystem 10 | `VerificationRequestPublisher` |

### Security Boundaries

```rust
// Only these subsystems can request peer lists
const PEER_LIST_SENDERS: [SubsystemId; 3] = [
    SubsystemId::BlockPropagation,   // 5
    SubsystemId::BloomFilters,       // 7
    SubsystemId::LightClients,       // 13
];

// Only Subsystem 10 can send verification results
// All other subsystems are REJECTED
```

## Usage

### Basic Usage

```rust
use qc_01_peer_discovery::{
    PeerDiscoveryService, PeerDiscoveryApi,
    NodeId, PeerInfo, SocketAddr, IpAddr, Timestamp,
    KademliaConfig, SystemTimeSource, TimeSource,
};

// Create service
let local_id = NodeId::new([0u8; 32]);
let config = KademliaConfig::default();
let time_source: Box<dyn TimeSource> = Box::new(SystemTimeSource);
let mut service = PeerDiscoveryService::new(local_id, config, time_source);

// Stage a new peer for verification
let peer = PeerInfo::new(
    NodeId::new([1u8; 32]),
    SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
    Timestamp::new(1000),
);
service.add_peer(peer.clone())?;

// After Subsystem 10 verifies the peer
let challenged = service.on_verification_result(&peer.node_id, true)?;

// Find closest peers to a target
let closest = service.find_closest_peers(target_id, 20);
```

### With Test Utilities

```rust
// Enable with `test-utils` feature
use qc_01_peer_discovery::FixedTimeSource;

let time_source = FixedTimeSource::new(1000);  // Fixed at timestamp 1000
```

## Features

| Feature | Description |
|---------|-------------|
| `default` | Core library, ZERO dependencies |
| `ipc` | Enables shared event bus integration (`shared-types`, `hmac`, `sha2`) |
| `rpc` | Enables API Gateway integration (`serde`, `serde_json`) |
| `bootstrap` | Enables bootstrap handler (`uuid`) |
| `network` | Enables `UdpNetworkSocket`, `TomlConfigProvider` (`tokio`, `toml`) |
| `test-utils` | Enables `FixedTimeSource` for testing |

## Dependencies

The core library has **ZERO** external dependencies. All integrations are optional.

```toml
[dependencies]
# Optional (feature-gated)
shared-types = { path = "../shared-types", optional = true }
hmac = { version = "0.12", optional = true }
sha2 = { version = "0.10", optional = true }
uuid = { version = "1.0", optional = true }
serde = { workspace = true, optional = true }
serde_json = { workspace = true, optional = true }
tokio = { workspace = true, optional = true }
toml = { version = "0.8", optional = true }
```

## Configuration

```rust
let config = KademliaConfig {
    k: 20,                              // Bucket size
    alpha: 3,                           // Lookup parallelism (used by node-runtime)
    max_peers_per_subnet: 2,            // IP diversity limit
    max_pending_peers: 1024,            // Memory bound (INVARIANT-9)
    verification_timeout_secs: 10,      // Pending peer timeout (INVARIANT-8)
    eviction_challenge_timeout_secs: 5, // Challenge timeout (INVARIANT-10)
};
```

## Testing

```bash
# Run all tests
cargo test -p qc-01-peer-discovery --all-features

# Run with verbose output
cargo test -p qc-01-peer-discovery --all-features -- --nocapture
```

## Test Coverage

| Test Group | Description | Ref |
|------------|-------------|-----|
| XOR Distance | Symmetric, correct bucket selection | SPEC-01 §5.1 Group 1 |
| K-Bucket | Size limits, eviction, challenges | SPEC-01 §5.1 Group 2 |
| IP Diversity | Subnet enforcement | SPEC-01 §5.1 Group 3 |
| Peer Lifecycle | Find, add, remove, ban | SPEC-01 §5.1 Group 4 |
| Ban System | Expiration, silent drop | SPEC-01 §5.1 Group 5 |
| Staging Area | Bounded staging, Tail Drop | SPEC-01 §5.1 Group 6, 7 |
| Eclipse Defense | Challenge flow | SPEC-01 §5.1 Group 8 |
| IPC Security | Authorization per IPC-MATRIX | IPC-MATRIX.md §1 |

## Related Documentation

- [`SPECS/SPEC-01-PEER-DISCOVERY.md`](../../SPECS/SPEC-01-PEER-DISCOVERY.md) - Full specification
- [`Documentation/Architecture.md`](../../Documentation/Architecture.md) - System architecture
- [`Documentation/IPC-MATRIX.md`](../../Documentation/IPC-MATRIX.md) - IPC authorization matrix
- [`Documentation/System.md`](../../Documentation/System.md) - Overall system design

## License

See repository LICENSE file.
