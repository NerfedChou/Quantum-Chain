# SPECIFICATION: PEER DISCOVERY & ROUTING

**Version:** 2.4  
**Subsystem ID:** 1  
**Bounded Context:** Network Topology & Peer Management  
**Crate Name:** `crates/peer-discovery`  
**Author:** Systems Architecture Team  
**Date:** 2024-12-01  
**Architecture Compliance:** Architecture.md v2.2 (Envelope-Only Identity, DDoS Edge Defense, Choreography Pattern, Bounded Staging, Eviction-on-Failure)

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
use std::collections::HashMap;

/// The main routing table implementing Kademlia DHT
/// 
/// SECURITY (DDoS Edge Defense - System.md Compliance):
/// New peers are staged in `pending_verification` until Subsystem 10 confirms
/// their identity. This prevents unverified peers from polluting the Kademlia table.
/// 
/// SECURITY (Bounded Staging - V2.3 Memory Bomb Defense):
/// The `pending_verification` HashMap is bounded by `config.max_pending_peers`.
/// When full, incoming peer requests are immediately dropped (Tail Drop Strategy).
/// This prevents memory exhaustion attacks. See INVARIANT-9.
pub struct RoutingTable {
    local_node_id: NodeId,
    buckets: [KBucket; 256],  // One bucket per bit distance
    banned_peers: BannedPeers,
    /// Staging area for peers awaiting signature verification from Subsystem 10.
    /// Peers move to `buckets` only after identity_valid == true.
    /// BOUNDED: Size limited by config.max_pending_peers (INVARIANT-9).
    pending_verification: HashMap<NodeId, PendingPeer>,
    /// Configuration including max_pending_peers limit.
    config: KademliaConfig,
}

/// A peer awaiting identity verification from Subsystem 10
pub struct PendingPeer {
    pub peer_info: PeerInfo,
    pub received_at: Timestamp,
    /// Timeout for verification (default: 10 seconds)
    pub verification_deadline: Timestamp,
}

/// A k-bucket storing up to k peers at a specific distance range
/// 
/// SECURITY (Eclipse Attack Defense - V2.4 Eviction-on-Failure):
/// When the bucket is full and a new verified peer wants to join, we do NOT
/// immediately evict the oldest peer. Instead, we CHALLENGE the oldest peer
/// with a PING. Only if the oldest peer fails to respond (is dead) do we evict.
/// This prevents "Table Poisoning" attacks where an attacker sequentially
/// connects with 20 new nodes to flush honest, stable peers.
pub struct KBucket {
    peers: Vec<PeerInfo>,      // Max size = K (20)
    last_updated: Timestamp,
    /// Peers waiting to join this bucket, pending eviction challenge result.
    /// When bucket is full and new peer arrives, old peer is challenged.
    /// If old peer responds (alive), new peer is rejected.
    /// If old peer times out (dead), new peer replaces it.
    pending_insertion: Option<PendingInsertion>,
}

/// A peer waiting to be inserted into a full bucket, pending challenge result
/// 
/// SECURITY (V2.4): This enables the "Eviction-on-Failure" policy.
/// The new peer only gets inserted if the challenged (oldest) peer is dead.
pub struct PendingInsertion {
    /// The new peer waiting to be inserted
    pub candidate: PeerInfo,
    /// The existing peer being challenged (oldest/least-recently-seen)
    pub challenged_peer: NodeId,
    /// When the challenge was sent
    pub challenge_sent_at: Timestamp,
    /// Deadline for challenge response (default: 5 seconds)
    pub challenge_deadline: Timestamp,
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

/// Reasons for banning a peer from the routing table
/// 
/// SECURITY (IP Spoofing Defense):
/// `InvalidSignature` is intentionally EXCLUDED from this enum.
/// In UDP contexts, IP addresses can be trivially spoofed. If we banned IPs
/// for bad signatures, an attacker could spoof a legitimate peer's IP (e.g., 
/// an exchange), send a bad signature, and trick us into banning the victim.
/// 
/// Instead, failed signature verification results in a SILENT DROP:
/// - Remove from `pending_verification`
/// - Do NOT add to `banned_peers`
/// - Log at DEBUG level only (no alerting)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BanReason {
    MalformedMessage,
    ExcessiveRequests,
    // InvalidSignature REMOVED - See security note above
    ManualBan,
}
```

### 2.3 Value Objects

```rust
/// Result of XOR distance calculation between two nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance(u8);  // 0-255 (which bit differs first)

/// Configuration constants for Kademlia
/// 
/// SECURITY (Bounded Staging - V2.3 Memory Bomb Defense):
/// The `max_pending_peers` field limits the size of the pending_verification
/// staging area. This prevents attackers from exhausting node memory by
/// flooding connection requests faster than signatures can be verified.
/// 
/// SECURITY (Eviction-on-Failure - V2.4 Eclipse Attack Defense):
/// The `eviction_challenge_timeout_secs` field controls how long we wait for
/// an oldest peer to respond before declaring it dead and allowing eviction.
pub struct KademliaConfig {
    pub k: usize,              // Bucket size (default: 20)
    pub alpha: usize,          // Parallelism (default: 3)
    pub max_peers_per_subnet: usize,  // IP diversity (default: 2)
    /// Maximum peers allowed in pending_verification staging area.
    /// Incoming requests beyond this limit are immediately dropped (Tail Drop).
    /// Default: 1024 (bounded memory: ~128KB worst case)
    pub max_pending_peers: usize,
    /// Timeout for eviction challenge PING (V2.4 Eclipse Defense).
    /// If oldest peer doesn't respond within this time, it's considered dead.
    /// Default: 5 seconds
    pub eviction_challenge_timeout_secs: u64,
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            k: 20,
            alpha: 3,
            max_peers_per_subnet: 2,
            max_pending_peers: 1024,
            eviction_challenge_timeout_secs: 5,
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

**INVARIANT-7: Pending Verification Staging (DDoS Edge Defense)**
```
∀ new_peer received from network:
    new_peer ∈ pending_verification UNTIL identity_valid == true from Subsystem 10
    new_peer ∉ buckets WHILE pending_verification
```

**INVARIANT-8: Verification Timeout**
```
∀ peer ∈ pending_verification:
    IF now > peer.verification_deadline THEN remove(peer) (silent drop)
```

**INVARIANT-9: Bounded Pending Verification (Memory Bomb Defense - V2.3)**
```
ALWAYS: pending_verification.len() ≤ max_pending_peers (default: 1024)

ENFORCEMENT (Tail Drop Strategy):
    IF pending_verification.len() >= max_pending_peers THEN
        IMMEDIATELY DROP incoming peer request
        DO NOT allocate memory for the new peer
        DO NOT evict existing pending peers
        Log at DEBUG level only (prevent log flooding)
    ENDIF

RATIONALE:
    - Prioritizes peers already undergoing verification (honest work)
    - Prevents attacker from flushing legitimate pending verifications
    - Bounds memory to: max_pending_peers × sizeof(PendingPeer) ≈ 128KB
    - Tail Drop is fair: first-come-first-served for staging slots
```

**INVARIANT-10: Eviction-on-Failure (Eclipse Attack Defense - V2.4)**
```
WHEN bucket.len() == K AND new_verified_peer wants to join:
    
    1. IDENTIFY oldest_peer = bucket.peers.min_by(last_seen)
    
    2. CHALLENGE oldest_peer with PING message
    
    3. WAIT for PONG response (timeout: 5 seconds)
    
    4. DECISION:
        IF oldest_peer responds (ALIVE):
            - Move oldest_peer to FRONT of bucket (most recent)
            - REJECT new_verified_peer
            - Rationale: Stable peers are more valuable than new peers
        
        IF oldest_peer times out (DEAD):
            - EVICT oldest_peer from bucket
            - INSERT new_verified_peer
            - Rationale: Only replace peers that are actually gone

SECURITY GUARANTEE:
    An attacker CANNOT displace healthy peers by simply connecting.
    To displace a peer, the attacker must:
    a) Wait for legitimate peer to go offline, OR
    b) Perform a network-level attack (which is out of scope)
    
    This makes "Table Poisoning" attacks mathematically infeasible
    against a healthy network mesh.

IMPLEMENTATION NOTE:
    Only ONE pending_insertion per bucket at a time.
    If a challenge is already in progress, new candidates are rejected.
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
    
    /// Add a newly discovered peer to the staging area for verification.
    /// 
    /// SECURITY (Bounded Staging - V2.3):
    /// This function enforces INVARIANT-9 (Memory Bomb Defense):
    /// - If pending_verification.len() >= max_pending_peers, returns Err(StagingAreaFull)
    /// - Peer is NOT added; request is immediately dropped (Tail Drop Strategy)
    /// - No eviction of existing pending peers (prioritizes honest work)
    /// 
    /// Returns Ok(true) if staged for verification, Ok(false) if rejected (banned, etc.)
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
    /// Current count of peers awaiting verification (V2.3)
    pub pending_verification_count: usize,
    /// Maximum allowed pending peers (V2.3)
    pub max_pending_peers: usize,
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
    /// V2.3: Staging area is at capacity (Memory Bomb Defense).
    /// Request immediately dropped; no memory allocated.
    /// See INVARIANT-9 for Tail Drop Strategy.
    StagingAreaFull,
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

**IMPORTANT:** All events in this section are **payloads** within the `AuthenticatedMessage<T>` envelope defined in Architecture.md Section 3.2. They are NOT standalone structs. Every IPC message MUST include the mandatory envelope fields: `version`, `correlation_id`, `reply_to`, `sender_id`, `recipient_id`, `timestamp`, `nonce`, and `signature`.

### 4.1 Events Published to Shared Bus

These are the payload types (`T` in `AuthenticatedMessage<T>`) that Peer Discovery publishes:

**ARCHITECTURAL CONTEXT (Stateful Assembler Pattern - Architecture.md v2.2):**

Some consumers of these events (notably Block Storage, Subsystem 2) implement the 
**Stateful Assembler** pattern. This means:

1. **Buffered Assembly:** Block Storage buffers events by `block_hash` key, waiting for 
   multiple components (BlockValidated, MerkleRootComputed, StateRootComputed) before 
   performing an atomic write.

2. **Timeout Behavior:** If all required components don't arrive within 30 seconds, the 
   incomplete assembly is dropped and logged as a warning.

3. **Implications for Publishers:** While Peer Discovery events are not part of the block 
   assembly flow, developers should understand that the event bus uses at-most-once 
   delivery for non-critical events. Critical events (like block-related data) are 
   handled via Dead Letter Queues (DLQ) per Architecture.md Section 5.3.

4. **No Orchestrator:** The system uses choreography, not orchestration. Each subsystem 
   reacts to events independently. There is no central coordinator.

```rust
/// Events emitted by the Peer Discovery subsystem
/// 
/// USAGE: These are payloads wrapped in AuthenticatedMessage<T>.
/// Example: AuthenticatedMessage<PeerConnectedPayload>
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryEventPayload {
    /// A new peer was successfully added to routing table
    PeerConnected(PeerConnectedPayload),
    
    /// A peer was removed from routing table
    PeerDisconnected(PeerDisconnectedPayload),
    
    /// A peer was banned
    PeerBanned(PeerBannedPayload),
    
    /// Bootstrap process completed
    BootstrapCompleted(BootstrapCompletedPayload),
    
    /// Routing table health warning
    RoutingTableWarning(RoutingTableWarningPayload),
    
    /// Response to a peer list request (correlated via correlation_id)
    PeerListResponse(PeerListResponsePayload),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerConnectedPayload {
    pub peer_info: PeerInfo,
    pub bucket_index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerDisconnectedPayload {
    pub node_id: NodeId,
    pub reason: DisconnectReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerBannedPayload {
    pub node_id: NodeId,
    pub reason: BanReason,
    pub duration_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapCompletedPayload {
    pub peer_count: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingTableWarningPayload {
    pub warning_type: WarningType,
    pub details: String,
}

/// Response payload for peer list requests
/// The correlation_id in the envelope links this to the original request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListResponsePayload {
    pub peers: Vec<PeerInfo>,
    pub total_available: usize,
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

### 4.2 Events Subscribed From Shared Bus (Request Payloads)

These are the payload types this subsystem listens for. All incoming messages MUST be validated against the `AuthenticatedMessage<T>` envelope requirements before processing.

```rust
/// Request payloads this subsystem handles
/// 
/// CRITICAL: These payloads arrive wrapped in AuthenticatedMessage<T>.
/// The envelope's correlation_id and reply_to fields MUST be used for responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerDiscoveryRequestPayload {
    /// Request for a list of known peers
    /// Allowed senders: Subsystems 5, 7, 13 ONLY
    PeerListRequest(PeerListRequestPayload),
    
    /// Request for full node connections (for light clients)
    /// Allowed senders: Subsystem 13 ONLY
    FullNodeListRequest(FullNodeListRequestPayload),
}

/// Request payload for peer list
/// 
/// SECURITY (Envelope-Only Identity - Architecture.md v2.2 Amendment 4.2):
/// This payload contains NO identity fields (e.g., `requester_id`).
/// The sender's identity is derived SOLELY from the AuthenticatedMessage
/// envelope's `sender_id` field, which is cryptographically signed.
/// This prevents "Payload Impersonation" attacks where an attacker could
/// spoof the payload identity while using their own envelope signature.
/// 
/// PRIVACY NOTE: The `filter` field, while not an identity field, can have
/// privacy implications. Complex or unique filter combinations may act as
/// fingerprints, allowing correlation of requests across sessions. Implementers
/// should consider offering standardized filter presets to reduce fingerprinting risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerListRequestPayload {
    pub max_peers: usize,
    /// Optional filter for peer selection.
    /// PRIVACY: Unique filter combinations may enable request fingerprinting.
    pub filter: Option<PeerFilter>,
}

/// Request payload for full node list (light clients)
/// 
/// NOTE: Identity derived from envelope.sender_id per Architecture.md v2.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullNodeListRequestPayload {
    pub max_nodes: usize,
    pub preferred_region: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerFilter {
    pub min_reputation: u8,
    pub exclude_subnets: Vec<SubnetMask>,
}
```

### 4.3 Request/Response Flow Example

Per Architecture.md Section 3.3, all request/response flows MUST use the correlation ID pattern. Here is a complete example:

```rust
// ============================================================
// REQUESTER SIDE (e.g., Block Propagation - Subsystem 5)
// ============================================================

impl BlockPropagation {
    /// Request peer list from Peer Discovery (NON-BLOCKING)
    async fn request_peer_list(&self) -> Result<(), Error> {
        // Step 1: Generate unique correlation ID
        let correlation_id = Uuid::new_v4();
        
        // Step 2: Store pending request for later matching
        self.pending_requests.insert(correlation_id, PendingRequest {
            created_at: Instant::now(),
            timeout: Duration::from_secs(30),
            request_type: RequestType::PeerList,
        });
        
        // Step 3: Construct the full authenticated message
        let message = AuthenticatedMessage {
            // === MANDATORY HEADER FIELDS ===
            version: PROTOCOL_VERSION,           // e.g., 1
            sender_id: SubsystemId::BlockPropagation,
            recipient_id: SubsystemId::PeerDiscovery,
            correlation_id: correlation_id.as_bytes().clone(),
            reply_to: Some(Topic {
                subsystem_id: SubsystemId::BlockPropagation,
                channel: "responses".into(),
            }),
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],                // Computed below
            
            // === PAYLOAD ===
            payload: PeerListRequestPayload {
                max_peers: 20,
                filter: Some(PeerFilter {
                    min_reputation: 30,
                    exclude_subnets: vec![],
                }),
            },
        };
        
        // Step 4: Sign the message
        let signed_message = message.sign(&self.shared_secret);
        
        // Step 5: Publish to event bus (NON-BLOCKING - returns immediately)
        self.event_bus.publish("peer-discovery.requests", signed_message).await?;
        
        // DO NOT AWAIT RESPONSE HERE - continue processing other work
        Ok(())
    }
    
    /// Handle responses from Peer Discovery (separate async handler)
    /// 
    /// IMPORTANT (Response Verification - Architecture.md v2.2):
    /// Response verification differs from request verification:
    /// - Responses do NOT require nonce cache checking (they are correlated, not deduplicated)
    /// - Responses MUST verify: signature, timestamp, sender_id, version
    /// - Correlation ID matching provides the anti-replay guarantee for responses
    async fn handle_peer_list_response(
        &mut self, 
        msg: AuthenticatedMessage<PeerListResponsePayload>
    ) {
        // Step 1: Verify protocol version FIRST (before any deserialization of payload)
        if msg.version < MIN_SUPPORTED_VERSION || msg.version > MAX_SUPPORTED_VERSION {
            log::warn!("Unsupported protocol version: {}", msg.version);
            return;
        }
        
        // Step 2: Verify sender is expected (must be PeerDiscovery for this response type)
        if msg.sender_id != SubsystemId::PeerDiscovery {
            log::warn!("Unexpected sender for PeerListResponse: {:?}", msg.sender_id);
            return;
        }
        
        // Step 3: Verify timestamp is within acceptable window (prevents stale responses)
        let now = self.time_source.now();
        let min_valid = now.saturating_sub(60);
        let max_valid = now.saturating_add(10);
        if msg.timestamp < min_valid || msg.timestamp > max_valid {
            log::warn!("Response timestamp out of range: {}", msg.timestamp);
            return;
        }
        
        // Step 4: Verify HMAC signature (cryptographic authenticity)
        // NOTE: We do NOT check nonce cache for responses - correlation_id provides anti-replay
        let computed_hmac = compute_hmac(&self.shared_secret, &msg.serialize_without_sig());
        if !constant_time_eq(&computed_hmac, &msg.signature) {
            log::warn!("Invalid signature on response from PeerDiscovery");
            return;
        }
        
        // Step 5: Match correlation_id to pending request (THIS is the anti-replay for responses)
        let correlation_id = Uuid::from_bytes(msg.correlation_id);
        if let Some(pending) = self.pending_requests.remove(&correlation_id) {
            // Step 6: Check if request timed out (response arrived too late)
            if pending.created_at.elapsed() > pending.timeout {
                log::warn!("Response arrived after timeout for {:?}", correlation_id);
                return;
            }
            
            // Step 7: Process the validated response
            for peer in msg.payload.peers {
                self.known_peers.insert(peer.node_id, peer);
            }
            
            log::debug!(
                "Received {} peers from PeerDiscovery (correlation: {:?})",
                msg.payload.peers.len(),
                correlation_id
            );
        } else {
            // Orphaned response - request already timed out, was cancelled, or never existed
            // This is normal in async systems; log at debug level only
            log::debug!("Orphaned response for correlation_id {:?}", correlation_id);
        }
    }
}

// ============================================================
// RESPONDER SIDE (Peer Discovery - Subsystem 1)
// ============================================================

impl PeerDiscovery {
    /// Handle incoming peer list requests
    async fn handle_peer_list_request(
        &self,
        msg: AuthenticatedMessage<PeerListRequestPayload>
    ) -> Result<(), Error> {
        // Step 1: Validate the envelope
        if let Err(e) = msg.verify_envelope() {
            return Err(Error::InvalidMessage(e));
        }
        
        // Step 2: Verify sender is authorized (Subsystems 5, 7, 13 only)
        match msg.sender_id {
            SubsystemId::BlockPropagation |
            SubsystemId::BloomFilters |
            SubsystemId::LightClients => { /* Authorized */ }
            _ => {
                return Err(Error::UnauthorizedSender(msg.sender_id));
            }
        }
        
        // Step 3: Process the request
        let peers = self.routing_table.get_random_peers(msg.payload.max_peers);
        
        // Step 4: Construct response with SAME correlation_id
        let response = AuthenticatedMessage {
            version: PROTOCOL_VERSION,
            sender_id: SubsystemId::PeerDiscovery,
            recipient_id: msg.sender_id,
            correlation_id: msg.correlation_id,      // CRITICAL: Same as request!
            reply_to: None,                          // This is a response, not a request
            timestamp: self.time_source.now(),
            nonce: self.nonce_generator.next(),
            signature: [0u8; 32],
            
            payload: PeerListResponsePayload {
                peers,
                total_available: self.routing_table.total_peer_count(),
            },
        };
        
        // Step 5: Sign and publish to the requester's reply_to topic
        let signed_response = response.sign(&self.shared_secret);
        let reply_topic = msg.reply_to
            .ok_or(Error::MissingReplyTo)?;
        
        self.event_bus.publish(&reply_topic.to_string(), signed_response).await?;
        
        Ok(())
    }
}
```

### 4.4 Message Envelope Compliance Checklist

For every IPC message sent or received by this subsystem:

| Field | Required | Validation |
|-------|----------|------------|
| `version` | ✅ YES | Must be within `[MIN_SUPPORTED_VERSION, MAX_SUPPORTED_VERSION]` |
| `sender_id` | ✅ YES | Must match expected sender per IPC Matrix |
| `recipient_id` | ✅ YES | Must be `SubsystemId::PeerDiscovery` for incoming |
| `correlation_id` | ✅ YES | UUID v4, used to match request/response pairs |
| `reply_to` | ✅ For requests | Topic where response should be published |
| `timestamp` | ✅ YES | Must be within 60 seconds of current time |
| `nonce` | ✅ For requests | Must not be reused (replay prevention via TimeBoundedNonceCache) |
| `signature` | ✅ YES | HMAC-SHA256, verified before processing |

**REQUEST vs RESPONSE Verification Differences:**

| Check | Request | Response |
|-------|---------|----------|
| Version | ✅ Required | ✅ Required |
| Sender ID | ✅ Required (per IPC Matrix) | ✅ Required (must be expected responder) |
| Timestamp | ✅ Required (60s window) | ✅ Required (60s window) |
| Signature | ✅ Required (HMAC) | ✅ Required (HMAC) |
| Nonce Cache | ✅ Required (TimeBoundedNonceCache) | ❌ NOT required |
| Correlation ID | ✅ Generate new UUID | ✅ Must match pending request |
| Reply-To | ✅ Required | ❌ Not applicable |

**Why Responses Skip Nonce Cache:**
Responses are correlated to a specific request via `correlation_id`. The anti-replay 
guarantee comes from the fact that each `correlation_id` can only be used once (it's 
removed from `pending_requests` upon first matching response). This is more efficient 
than maintaining a separate nonce cache for responses.

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
fn test_bucket_rejects_21st_peer_when_full_and_all_alive()
// Verify: INVARIANT-1 (bucket size ≤ 20)
// Scenario: Fill bucket with 20 peers, all respond to PING
// Assert: 21st peer is REJECTED, bucket remains at 20

#[test]
fn test_bucket_prefers_stable_peers_over_new_peers()
// Verify: INVARIANT-10 (Eviction-on-Failure) - Eclipse Attack Defense
// Scenario: Fill bucket with 20 peers. Add 21st peer.
// Action: Simulate "Oldest Peer is ALIVE" (responds to PING challenge)
// Assert: 21st peer is REJECTED
// Assert: Oldest peer remains in bucket and is moved to front (most recent)
// Rationale: Stable peers are more valuable than new peers

#[test]
fn test_bucket_evicts_dead_peers_for_new_peers()
// Verify: INVARIANT-10 (Eviction-on-Failure) - Dead peer replacement
// Scenario: Fill bucket with 20 peers. Add 21st peer.
// Action: Simulate "Oldest Peer is DEAD" (times out on PING challenge)
// Assert: Oldest peer is EVICTED
// Assert: 21st peer is INSERTED
// Rationale: Only replace peers that are actually gone

#[test]
fn test_bucket_challenge_in_progress_rejects_additional_candidates()
// Verify: Only ONE pending_insertion per bucket at a time
// Scenario: Bucket full, peer A triggers challenge, peer B arrives during challenge
// Assert: Peer B is immediately rejected (challenge already in progress)

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

#### Test Group 6: Pending Verification Staging (DDoS Edge Defense)

```rust
#[test]
fn test_new_peer_goes_to_pending_verification_first()
// Verify: INVARIANT-7 - new peer enters staging, not buckets

#[test]
fn test_peer_moves_to_buckets_after_identity_valid_true()
// Verify: On NodeIdentityVerificationResult with identity_valid=true,
//         peer moves from pending_verification to appropriate bucket

#[test]
fn test_peer_silently_dropped_on_identity_valid_false()
// Verify: On identity_valid=false, peer removed from pending_verification
//         WITHOUT being added to banned_peers (IP spoofing defense)

#[test]
fn test_pending_peer_times_out_after_deadline()
// Verify: INVARIANT-8 - peer removed if verification not received in time

#[test]
fn test_invalid_signature_does_not_trigger_ban()
// Verify: BanReason::InvalidSignature removed; bad sig = silent drop only
```

#### Test Group 7: Bounded Staging (Memory Bomb Defense - V2.3)

```rust
#[test]
fn test_staging_area_rejects_peer_when_at_capacity()
// Verify: INVARIANT-9 - when pending_verification.len() == max_pending_peers,
//         add_peer returns Err(StagingAreaFull) and does NOT allocate memory

#[test]
fn test_staging_area_uses_tail_drop_not_eviction()
// Verify: When staging is full, existing pending peers are NOT evicted
//         New peer is dropped, not existing ones (prioritize honest work)

#[test]
fn test_staging_area_accepts_peer_below_capacity()
// Verify: When pending_verification.len() < max_pending_peers,
//         add_peer succeeds and peer enters staging

#[test]
fn test_staging_area_capacity_freed_after_verification_complete()
// Verify: After identity_valid received (true or false), staging slot is freed
//         New peers can now be accepted

#[test]
fn test_staging_area_capacity_freed_after_timeout()
// Verify: After verification_deadline passes, staging slot is freed via GC

#[test]
fn test_get_stats_reports_pending_verification_count()
// Verify: RoutingTableStats.pending_verification_count matches actual count
//         and max_pending_peers matches config
```

#### Test Group 8: Eviction-on-Failure (Eclipse Attack Defense - V2.4)

```rust
#[test]
fn test_eviction_challenge_is_sent_when_bucket_full()
// Verify: When bucket is full and new verified peer arrives,
//         a PING challenge is sent to the oldest peer (not immediate eviction)

#[test]
fn test_alive_peer_is_moved_to_front_after_challenge()
// Verify: INVARIANT-10 - When oldest peer responds to PING,
//         it is moved to front of bucket (most recently seen)

#[test]
fn test_new_peer_rejected_when_oldest_is_alive()
// Verify: INVARIANT-10 - Stable peers preferred over new peers
// This is the core Eclipse Attack defense

#[test]
fn test_dead_peer_evicted_after_challenge_timeout()
// Verify: INVARIANT-10 - When oldest peer fails to respond within 5s,
//         it is evicted and new peer is inserted

#[test]
fn test_challenge_timeout_is_configurable()
// Verify: Challenge deadline can be configured (default: 5 seconds)

#[test]
fn test_only_one_pending_insertion_per_bucket()
// Verify: If challenge already in progress, new candidates are rejected
//         Prevents memory exhaustion via pending_insertion flooding

#[test]
fn test_table_poisoning_attack_is_blocked()
// Verify: Attacker cannot flush 20 honest peers by connecting 20 new nodes
// Scenario: 
//   1. Fill bucket with 20 "honest" peers (all respond to PING)
//   2. Attacker sequentially tries to add 20 "malicious" peers
// Assert: All 20 honest peers remain; all 20 malicious peers rejected
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

**Attack Vector (Table Poisoning):**
```
1. Attacker identifies target node
2. Attacker connects 20 malicious nodes to target's bucket
3. With "Naive Eviction" (VULNERABLE): Each new node evicts oldest honest peer
4. Result: Bucket filled with 100% attacker-controlled nodes
5. Attacker can now censor transactions, feed false chain data, etc.
```

**Defense: Eviction-on-Failure (V2.4 - INVARIANT-10)**

| Step | Action | Outcome |
|------|--------|---------|
| 1 | Attacker's node arrives, bucket is full | Challenge sent to oldest peer |
| 2 | Oldest honest peer receives PING | Responds with PONG |
| 3 | Oldest peer is ALIVE | Attacker's node is REJECTED |
| 4 | Oldest peer moved to front | Now "most recently seen" |

**Why This Works:**
- Honest peers that are online CANNOT be displaced
- Attacker must wait for honest peers to naturally go offline
- 20-node attack requires 20 honest peers to die → infeasible against healthy network

**Additional Mitigations:**
1. **Bootstrap Node Diversity:** Require connections to multiple bootstrap nodes from different entities
2. **Outbound Connection Limits:** Maintain at least 8 outbound connections to random peers
3. **Random Peer Selection:** Don't only connect to "closest" peers (deterministic → vulnerable)

### 6.3 Memory Constraints

**Limits:**
- **Routing Table Size:** 256 buckets × 20 peers = **5,120 peers maximum**
- **Memory Per Peer:** ~128 bytes (NodeId + SocketAddr + metadata)
- **Total Memory:** ~640 KB for routing table (acceptable)
- **Pending Verification (V2.3):** max_pending_peers × ~128 bytes = **~128 KB maximum**

**Enforcement:**
```rust
const MAX_TOTAL_PEERS: usize = 5120;

fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
    // V2.3: Check staging area capacity FIRST (Memory Bomb Defense)
    // This check is O(1) and prevents ANY memory allocation if staging is full
    if self.pending_verification.len() >= self.config.max_pending_peers {
        // Tail Drop: Immediately reject, do not allocate, do not evict
        log::debug!("Staging area full, dropping incoming peer request");
        return Err(PeerDiscoveryError::StagingAreaFull);
    }
    
    if self.total_peer_count() >= MAX_TOTAL_PEERS {
        return Err(PeerDiscoveryError::RoutingTableFull);
    }
    // ... rest of logic (stage in pending_verification)
}
```

### 6.4 Rate Limiting (DoS Prevention)

**Constraint:** Do not respond to more than 10 PING requests per second from same IP.

**Implementation Note:** This is handled by the `NetworkSocket` adapter, not domain logic.

### 6.5 Memory Bomb Defense (V2.3)

**Threat:** Attacker floods node with connection requests faster than signatures can be verified, exhausting memory.

**Attack Vector:**
```
Attacker sends 100,000 connection requests per second.
Each request creates a PendingPeer (~128 bytes) in staging area.
Without bounds: 100K × 128 bytes = 12.8 MB/second memory growth.
Node runs out of memory within minutes → crash → network partition.
```

**Defense: Bounded Staging with Tail Drop (INVARIANT-9)**

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `max_pending_peers` | 1024 (default) | ~128 KB max memory |
| Strategy | Tail Drop | Prioritize existing honest work |
| Eviction | NONE | Attackers cannot flush legitimate peers |

**Why Tail Drop, Not Eviction:**
- **Eviction** (remove oldest to make room) allows attacker to flush honest peers
- **Tail Drop** (reject new when full) preserves peers already being verified
- Honest peers who arrived first get verified first (fair)
- Attacker cannot disrupt in-progress verifications

**Implementation:**
```rust
// CRITICAL: This check MUST be the FIRST operation in add_peer()
// It is O(1) and prevents ANY memory allocation on the hot path
if self.pending_verification.len() >= self.config.max_pending_peers {
    return Err(PeerDiscoveryError::StagingAreaFull);
}
```

### 6.6 Panic Policy

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
- **Architecture Document:** Section 3.2 (AuthenticatedMessage envelope), Section 3.3 (Request/Response Correlation Pattern), Section 4.1 (Subsystem Catalog - Peer Discovery)
- **Kademlia Paper:** Maymounkov & Mazières (2002) - "Kademlia: A Peer-to-peer Information System Based on the XOR Metric"

### 7.4 Related Specifications

- **SPEC-05-BLOCK-PROPAGATION.md** (depends on this subsystem for peer lists)
- **SPEC-07-BLOOM-FILTERS.md** (depends on this subsystem for full node discovery)
- **SPEC-13-LIGHT-CLIENT.md** (depends on this subsystem for full node connections)

---

## 8. IMPLEMENTATION CHECKLIST

### Phase 1: Domain Logic (Pure)
- [ ] Implement `NodeId` type with XOR distance calculation
- [ ] Implement `KBucket` with size limit enforcement and `PendingInsertion` for challenges
- [ ] Implement `PendingPeer` staging area for unverified peers
- [ ] Implement `RoutingTable` with all invariants (INVARIANT-7 through INVARIANT-10)
- [ ] Implement bounded staging with Tail Drop (max_pending_peers check in add_peer)
- [ ] Implement Eviction-on-Failure challenge flow (V2.4 Eclipse Attack Defense)
- [ ] Implement `find_closest_peers` algorithm
- [ ] Implement `BannedPeers` with expiration logic
- [ ] Implement silent drop logic for failed signature verification (NO ban)
- [ ] Write all TDD tests from Section 5.1 (including Test Groups 6, 7, and 8)

### Phase 2: Port Definitions
- [ ] Define `PeerDiscoveryApi` trait
- [ ] Define `NetworkSocket` trait
- [ ] Define `TimeSource` trait
- [ ] Define `ConfigProvider` trait
- [ ] Define `NodeIdValidator` trait

### Phase 3: Event Integration
- [ ] Define `PeerDiscoveryEventPayload` enum (payloads only, not standalone)
- [ ] Define `PeerDiscoveryRequestPayload` enum for incoming requests
- [ ] Implement `AuthenticatedMessage<T>` envelope handling
- [ ] Implement correlation ID tracking for request/response flows
- [ ] Implement event publishing on peer lifecycle changes
- [ ] Implement event subscription with sender validation per IPC Matrix
- [ ] Implement response routing via `reply_to` topic

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