# SPECIFICATION: PEER DISCOVERY & ROUTING

**Version:** 1.0  
**Subsystem ID:** 1  
**Bounded Context:** Network Topology & Peer Management  
**Crate Name:** `crates/peer-discovery`  
**Author:** Systems Architecture Team  
**Date:** 2024-11-30

---

## 1. ABSTRACT

### 1.1 Purpose

The **Peer Discovery** subsystem is responsible for maintaining a distributed, self-organizing network of blockchain nodes. It implements the Kademlia Distributed Hash Table (DHT) algorithm to efficiently locate and manage peer connections across the network.

### 1.2 Responsibility Boundaries

**In Scope:**
- Maintain a routing table of known peers organized by XOR distance
- Perform iterative node lookups to find closest peers to any target ID
- Manage peer lifecycle (discovery, connection, disconnection, banning)
- Enforce network topology constraints (IP diversity, bucket limits)
- Provide peer lists to other subsystems (Block Propagation, Light Clients)

**Out of Scope:**
- Actual network I/O (delegated to `NetworkSocket` adapter)
- Block or transaction propagation (handled by Subsystem 5)
- Consensus or validation logic
- Persistent storage of routing table (optional, delegated to adapter)

### 1.3 Key Design Principles

1. **Pure Domain Logic:** All Kademlia logic (XOR distance, bucket management) is pure Rust with no I/O
2. **Adapter Agnostic:** Works with any transport (UDP, TCP, QUIC) via `NetworkSocket` port
3. **Deterministic:** Given same inputs, produces same routing decisions (testable)
4. **Security-First:** IP diversity and Sybil resistance built into core logic

---

## 2. DOMAIN MODEL (THE "INNER LAYER")

### 2.1 Core Entities

```rust
/// 256-bit node identifier derived from public key hash
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId([u8; 32]);

/// Complete peer information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub socket_addr: SocketAddr,
    pub last_seen: Timestamp,
    pub reputation_score: u8,  // 0-100, starts at 50
}

/// Socket address (IP + Port) - abstraction over std::net::SocketAddr
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SocketAddr {
    pub ip: IpAddr,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddr {
    V4([u8; 4]),
    V6([u8; 16]),
}

/// Unix timestamp in seconds
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);
```

### 2.2 Routing Table Structure

```rust
/// The main routing table implementing Kademlia DHT
pub struct RoutingTable {
    local_node_id: NodeId,
    buckets: [KBucket; 256],  // One bucket per bit distance
    banned_peers: BannedPeers,
}

/// A k-bucket storing up to k peers at a specific distance range
pub struct KBucket {
    peers: Vec<PeerInfo>,      // Max size = K (20)
    last_updated: Timestamp,
}

/// Tracks banned peers with expiration times
pub struct BannedPeers {
    entries: Vec<BannedEntry>,
}

pub struct BannedEntry {
    node_id: NodeId,
    banned_until: Timestamp,
    reason: BanReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanReason {
    MalformedMessage,
    ExcessiveRequests,
    InvalidSignature,
    ManualBan,
}
```

### 2.3 Value Objects

```rust
/// Result of XOR distance calculation between two nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance(u8);  // 0-255 (which bit differs first)

/// Configuration constants for Kademlia
pub struct KademliaConfig {
    pub k: usize,              // Bucket size (default: 20)
    pub alpha: usize,          // Parallelism (default: 3)
    pub max_peers_per_subnet: usize,  // IP diversity (default: 2)
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            k: 20,
            alpha: 3,
            max_peers_per_subnet: 2,
        }
    }
}

/// Subnet mask for IP diversity checks
#[derive(Debug, Clone, Copy)]
pub struct SubnetMask {
    pub prefix_length: u8,  // e.g., 24 for /24
}
```

### 2.4 Domain Invariants

**INVARIANT-1: Bucket Size Limit**
```
∀ bucket ∈ RoutingTable.buckets:
    bucket.peers.len() ≤ K (20)
```

**INVARIANT-2: Local Node Immutability**
```
Once RoutingTable is created, RoutingTable.local_node_id NEVER changes
```

**INVARIANT-3: IP Diversity**
```
∀ bucket ∈ RoutingTable.buckets:
    peers_in_same_subnet(bucket, subnet_mask) ≤ max_peers_per_subnet
```

**INVARIANT-4: Banned Peer Exclusion**
```
∀ peer ∈ RoutingTable.buckets[*].peers:
    peer.node_id ∉ BannedPeers
```

**INVARIANT-5: Self-Exclusion**
```
∀ peer ∈ RoutingTable.buckets[*].peers:
    peer.node_id ≠ RoutingTable.local_node_id
```

**INVARIANT-6: Distance Ordering**
```
∀ peer ∈ RoutingTable.buckets[i]:
    distance(local_node_id, peer.node_id).leading_zeros() == i
```

---

## 3. PORTS & INTERFACES (THE "HEXAGON")

### 3.1 Driving Ports (Inbound API)

These are the public APIs this library exposes to the application node.

```rust
/// Primary API for interacting with the peer discovery subsystem
pub trait PeerDiscoveryApi {
    /// Find the k closest peers to a target ID
    /// Used for iterative node lookups
    fn find_closest_peers(
        &self, 
        target_id: NodeId, 
        count: usize
    ) -> Vec<PeerInfo>;
    
    /// Add a newly discovered peer to the routing table
    /// Returns Ok(true) if added, Ok(false) if rejected (full bucket, banned, etc.)
    fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError>;
    
    /// Get a random selection of peers (for gossip protocols)
    /// Used by Subsystem 5 (Block Propagation)
    fn get_random_peers(&self, count: usize) -> Vec<PeerInfo>;
    
    /// Manually ban a peer for a duration
    fn ban_peer(
        &mut self, 
        node_id: NodeId, 
        duration_seconds: u64, 
        reason: BanReason
    ) -> Result<(), PeerDiscoveryError>;
    
    /// Check if a peer is currently banned
    fn is_banned(&self, node_id: NodeId) -> bool;
    
    /// Update peer's last-seen timestamp (keep-alive)
    fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError>;
    
    /// Remove a peer from routing table (due to timeout, error, etc.)
    fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError>;
    
    /// Get current routing table statistics
    fn get_stats(&self) -> RoutingTableStats;
}

/// Statistics about the routing table state
pub struct RoutingTableStats {
    pub total_peers: usize,
    pub buckets_used: usize,
    pub banned_count: usize,
    pub oldest_peer_age_seconds: u64,
}

/// Errors that can occur during peer discovery operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryError {
    PeerNotFound,
    PeerBanned,
    BucketFull,
    InvalidNodeId,
    SubnetLimitReached,
    SelfConnection,  // Attempting to add local node
}
```

### 3.2 Driven Ports (Outbound SPI)

These are the interfaces this library **requires** the host application to implement.

```rust
/// Abstract interface for network I/O
/// The host must provide a concrete implementation (e.g., using tokio UDP)
pub trait NetworkSocket: Send + Sync {
    /// Send a PING message to a peer
    fn send_ping(&self, target: SocketAddr) -> Result<(), NetworkError>;
    
    /// Send a FIND_NODE query to a peer
    fn send_find_node(
        &self, 
        target: SocketAddr, 
        search_id: NodeId
    ) -> Result<(), NetworkError>;
    
    /// Send a PONG response to a peer
    fn send_pong(&self, target: SocketAddr) -> Result<(), NetworkError>;
}

#[derive(Debug)]
pub enum NetworkError {
    Timeout,
    ConnectionRefused,
    InvalidAddress,
    MessageTooLarge,
}

/// Abstract interface for time-related operations
/// Allows testing with mock time
pub trait TimeSource: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// Abstract interface for configuration loading
pub trait ConfigProvider: Send + Sync {
    /// Get list of bootstrap nodes to connect to initially
    fn get_bootstrap_nodes(&self) -> Vec<SocketAddr>;
    
    /// Get Kademlia configuration parameters
    fn get_kademlia_config(&self) -> KademliaConfig;
}

/// Abstract interface for proof-of-work validation (Sybil resistance)
pub trait NodeIdValidator: Send + Sync {
    /// Verify that a NodeId has sufficient proof-of-work
    /// (e.g., leading zeros in hash)
    fn validate_node_id(&self, node_id: NodeId) -> bool;
}
```

---

## 4. EVENT SCHEMA (EDA)

### 4.1 Events Published to Shared Bus

```rust
/// Events emitted by the Peer Discovery subsystem
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryEvent {
    /// A new peer was successfully added to routing table
    PeerConnected {
        peer_info: PeerInfo,
        bucket_index: u8,
    },
    
    /// A peer was removed from routing table
    PeerDisconnected {
        node_id: NodeId,
        reason: DisconnectReason,
    },
    
    /// A peer was banned
    PeerBanned {
        node_id: NodeId,
        reason: BanReason,
        duration_seconds: u64,
    },
    
    /// Bootstrap process completed
    /// (Sufficient peers found to consider network joined)
    BootstrapCompleted {
        peer_count: usize,
        duration_ms: u64,
    },
    
    /// Routing table health warning
    /// (Too few peers, buckets empty, etc.)
    RoutingTableWarning {
        warning_type: WarningType,
        details: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Timeout,
    ExplicitRemoval,
    BucketReplacement,
    NetworkError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningType {
    TooFewPeers,
    NoRecentActivity,
    HighChurnRate,
}
```

### 4.2 Events Subscribed From Shared Bus

```rust
/// Events this subsystem listens to from other subsystems
pub enum IncomingEvent {
    /// From Subsystem 5 (Block Propagation) - requesting peer list
    PeerListRequested {
        requester_id: SubsystemId,
        request_id: u64,
    },
    
    /// From Subsystem 7 (Bloom Filters) - requesting full nodes
    FullNodeListRequested {
        requester_id: SubsystemId,
    },
    
    /// From Subsystem 13 (Light Clients) - requesting full nodes
    LightClientPeerRequest {
        requester_id: SubsystemId,
    },
}
```

---

## 5. TDD VALIDATION STRATEGY

### 5.1 Critical Domain Logic Tests (Red Phase)

Before implementing any function bodies, we must write these failing tests:

#### Test Group 1: XOR Distance Calculation

```rust
#[test]
fn test_xor_distance_calculation_is_symmetric()
// Verify: distance(A, B) == distance(B, A)

#[test]
fn test_xor_distance_to_self_is_zero()
// Verify: distance(A, A) == 0

#[test]
fn test_xor_distance_identifies_correct_bucket()
// Verify: Node with 1 bit different → bucket 0
//         Node with 2 bits different → bucket 1, etc.

#[test]
fn test_xor_distance_ordering_for_closest_peers()
// Verify: Given peers P1, P2, P3 with distances D1 < D2 < D3
//         find_closest_peers returns [P1, P2, P3]
```

#### Test Group 2: K-Bucket Management

```rust
#[test]
fn test_bucket_rejects_21st_peer_when_full()
// Verify: INVARIANT-1 (bucket size ≤ 20)

#[test]
fn test_bucket_replaces_least_recently_seen_peer()
// Verify: When bucket full, new peer replaces oldest

#[test]
fn test_bucket_rejects_peer_if_banned()
// Verify: INVARIANT-4 (banned peers excluded)

#[test]
fn test_bucket_rejects_self_node()
// Verify: INVARIANT-5 (no self-connection)

#[test]
fn test_bucket_maintains_distance_ordering()
// Verify: INVARIANT-6 (peers in correct bucket by distance)
```

#### Test Group 3: IP Diversity Enforcement

```rust
#[test]
fn test_rejects_third_peer_from_same_subnet()
// Verify: INVARIANT-3 with max_peers_per_subnet = 2

#[test]
fn test_allows_peers_from_different_subnets()
// Verify: Peers from different /24 subnets accepted

#[test]
fn test_subnet_check_works_for_ipv6()
// Verify: IPv6 subnet diversity enforced
```

#### Test Group 4: Peer Lifecycle

```rust
#[test]
fn test_find_closest_peers_returns_k_peers()
// Verify: With 50 peers, find_closest returns 20

#[test]
fn test_find_closest_peers_returns_sorted_by_distance()
// Verify: Returned peers ordered by XOR distance

#[test]
fn test_banned_peer_not_included_in_closest_peers()
// Verify: Banned peers excluded from search results

#[test]
fn test_get_random_peers_returns_diverse_selection()
// Verify: Random selection not biased to single bucket
```

#### Test Group 5: Ban System

```rust
#[test]
fn test_banned_peer_expires_after_duration()
// Verify: Peer banned for 60s is unbanned after 60s

#[test]
fn test_is_banned_returns_true_during_ban_period()
// Verify: is_banned(peer) == true while ban active

#[test]
fn test_cannot_add_banned_peer_to_routing_table()
// Verify: add_peer fails with PeerDiscoveryError::PeerBanned
```

### 5.2 Integration Tests (Port Contracts)

```rust
#[test]
fn test_network_socket_adapter_sends_ping()
// Verify: NetworkSocket implementation actually sends UDP packet

#[test]
fn test_time_source_adapter_returns_monotonic_time()
// Verify: TimeSource always returns increasing timestamps

#[test]
fn test_config_provider_loads_bootstrap_nodes()
// Verify: ConfigProvider can parse config.toml
```

---

## 6. SECURITY & CONSTRAINTS

### 6.1 Sybil Attack Resistance

**Threat:** Attacker creates thousands of fake NodeIDs to take over routing table.

**Mitigations:**
1. **NodeId Validation:** Require proof-of-work (configurable leading zeros)
   ```rust
   // Example: NodeId must have 16 leading zero bits
   fn validate_node_id(id: NodeId) -> bool {
       id.0[0] == 0 && id.0[1] == 0
   }
   ```

2. **IP Diversity:** Enforce `max_peers_per_subnet` (default: 2)
    - Prevents single IP from dominating routing table

3. **Reputation Scoring:** Track peer behavior (successful responses, failures)
    - New peers start at reputation_score = 50
    - Malicious behavior decreases score → automatic ban at score < 10

### 6.2 Eclipse Attack Resistance

**Threat:** Attacker isolates victim by controlling all its connections.

**Mitigations:**
1. **Bootstrap Node Diversity:** Require connections to multiple bootstrap nodes from different entities
2. **Outbound Connection Limits:** Maintain at least 8 outbound connections to random peers
3. **Random Peer Selection:** Don't only connect to "closest" peers (deterministic → vulnerable)

### 6.3 Memory Constraints

**Limits:**
- **Routing Table Size:** 256 buckets × 20 peers = **5,120 peers maximum**
- **Memory Per Peer:** ~128 bytes (NodeId + SocketAddr + metadata)
- **Total Memory:** ~640 KB for routing table (acceptable)

**Enforcement:**
```rust
const MAX_TOTAL_PEERS: usize = 5120;

fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
    if self.total_peer_count() >= MAX_TOTAL_PEERS {
        return Err(PeerDiscoveryError::RoutingTableFull);
    }
    // ... rest of logic
}
```

### 6.4 Rate Limiting (DoS Prevention)

**Constraint:** Do not respond to more than 10 PING requests per second from same IP.

**Implementation Note:** This is handled by the `NetworkSocket` adapter, not domain logic.

### 6.5 Panic Policy

**Principle:** This library must NEVER panic in production.

**Rules:**
1. All array accesses use `.get()` with `Result` return
2. All integer operations checked for overflow
3. All unwrap() calls replaced with proper error handling

```rust
// ❌ FORBIDDEN
let peer = self.buckets[index].peers[0];  // Can panic

// ✅ REQUIRED
let peer = self.buckets
    .get(index)
    .and_then(|bucket| bucket.peers.first())
    .ok_or(PeerDiscoveryError::PeerNotFound)?;
```

---

## 7. DEPENDENCIES & REFERENCES

### 7.1 Internal Dependencies

- **Shared Types Crate** (`crates/shared-types`):
    - `SubsystemId` enum
    - `Signature` type
    - Common error types

- **Shared Bus Crate** (`crates/shared-bus`):
    - `EventPublisher` trait
    - `EventSubscriber` trait
    - `BlockchainEvent` enum

### 7.2 External Crate Dependencies (Minimal)

```toml
[dependencies]
# Cryptographic operations (if NodeId validation needed)
# sha3 = "0.10"  # Only if validating NodeIds via hash

# No other dependencies allowed in domain layer
```

### 7.3 References

- **IPC Matrix Document:** See message types for `PeerListRequest` and `PeerList`
- **Architecture Document:** Section 4.1 (Subsystem Catalog - Peer Discovery)
- **Kademlia Paper:** Maymounkov & Mazières (2002) - "Kademlia: A Peer-to-peer Information System Based on the XOR Metric"

### 7.4 Related Specifications

- **SPEC-05-BLOCK-PROPAGATION.md** (depends on this subsystem for peer lists)
- **SPEC-07-BLOOM-FILTERS.md** (depends on this subsystem for full node discovery)
- **SPEC-13-LIGHT-CLIENT.md** (depends on this subsystem for full node connections)

---

## 8. IMPLEMENTATION CHECKLIST

### Phase 1: Domain Logic (Pure)
- [ ] Implement `NodeId` type with XOR distance calculation
- [ ] Implement `KBucket` with size limit enforcement
- [ ] Implement `RoutingTable` with all invariants
- [ ] Implement `find_closest_peers` algorithm
- [ ] Implement `BannedPeers` with expiration logic
- [ ] Write all TDD tests from Section 5.1

### Phase 2: Port Definitions
- [ ] Define `PeerDiscoveryApi` trait
- [ ] Define `NetworkSocket` trait
- [ ] Define `TimeSource` trait
- [ ] Define `ConfigProvider` trait
- [ ] Define `NodeIdValidator` trait

### Phase 3: Event Integration
- [ ] Define `PeerDiscoveryEvent` enum
- [ ] Implement event publishing on peer lifecycle changes
- [ ] Implement event subscription for peer list requests

### Phase 4: Adapters (Separate Crate)
- [ ] Create `peer-discovery-adapters` crate
- [ ] Implement `UdpNetworkSocket` using tokio
- [ ] Implement `SystemTimeSource`
- [ ] Implement `TomlConfigProvider`
- [ ] Write integration tests

---

## 9. OPEN QUESTIONS & DESIGN DECISIONS

### Q1: Persistent Routing Table?
**Question:** Should routing table be persisted to disk between restarts?

**Options:**
- A) No persistence (rebuild from bootstrap on restart)
- B) Optional persistence via `RoutingTableStorage` port

**Decision:** Defer to implementation phase. Port defined, but not required.

### Q2: NodeId Generation
**Question:** How is local NodeId generated? From private key hash?

**Decision:** Out of scope for this library. NodeId provided at construction time.

### Q3: IPv4 vs IPv6 Priority
**Question:** Should we prefer IPv6 peers over IPv4?

**Decision:** No preference. Treat equally unless configurable.

---

## 10. ACCEPTANCE CRITERIA

This specification is considered **complete** when:

1. ✅ All domain entities defined with no implementation
2. ✅ All invariants explicitly stated
3. ✅ All ports (Driving + Driven) defined as traits
4. ✅ All events defined for Shared Bus
5. ✅ All TDD tests listed (names only, no code)
6. ✅ Security constraints documented
7. ✅ Memory limits specified
8. ✅ Panic policy stated

This specification is considered **approved** when:

1. ✅ Reviewed by senior architect
2. ✅ Confirmed to match IPC Matrix requirements
3. ✅ Confirmed to follow hexagonal architecture
4. ✅ No implementation code present (only signatures)

---

**END OF SPECIFICATION**

**Next Steps:**
1. Review this specification for completeness
2. Approve for TDD Phase (write tests)
3. Implement domain logic (pass tests)
4. Implement adapters (wire to runtime)