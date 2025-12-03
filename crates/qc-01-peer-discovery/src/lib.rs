//! # Peer Discovery & Routing Subsystem
//!
//! **Subsystem ID:** 1  
//! **Specification:** `SPECS/SPEC-01-PEER-DISCOVERY.md` v2.4
//!
//! This crate implements the Kademlia Distributed Hash Table (DHT) for
//! peer discovery and routing in the Quantum-Chain network.
//!
//! ## Architecture
//!
//! The crate follows Hexagonal Architecture with:
//! - **Domain Layer:** Pure Kademlia logic (XOR distance, k-buckets, routing table)
//! - **Ports Layer:** Trait definitions for external dependencies
//! - **Adapters Layer:** Concrete implementations for IPC and Event Bus
//!
//! ## Security Features
//!
//! - **DDoS Edge Defense:** New peers staged for verification via Subsystem 10
//! - **Memory Bomb Defense (V2.3):** Bounded staging with Tail Drop strategy
//! - **Eclipse Attack Defense (V2.4):** Eviction-on-Failure policy
//! - **IP Diversity:** Subnet-based limits to prevent Sybil attacks
//!
//! ## Example
//!
//! ```rust
//! use qc_01_peer_discovery::domain::{
//!     NodeId, PeerInfo, SocketAddr, IpAddr, Timestamp,
//!     KademliaConfig, RoutingTable,
//! };
//!
//! // Create local node
//! let local_id = NodeId::new([0u8; 32]);
//! let config = KademliaConfig::default();
//! let mut table = RoutingTable::new(local_id, config);
//!
//! // Stage a peer for verification
//! let peer = PeerInfo::new(
//!     NodeId::new([1u8; 32]),
//!     SocketAddr::new(IpAddr::v4(192, 168, 1, 100), 8080),
//!     Timestamp::new(1000),
//! );
//! let now = Timestamp::new(1000);
//!
//! // Stage peer (awaiting Subsystem 10 verification)
//! table.stage_peer(peer.clone(), now).unwrap();
//!
//! // After Subsystem 10 verifies, promote to routing table
//! table.on_verification_result(&peer.node_id, true, now).unwrap();
//! ```

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;
pub mod service;

// Re-export commonly used types
pub use domain::{
    BanReason, DisconnectReason, Distance, IpAddr, KBucket, KademliaConfig, NodeId,
    PeerDiscoveryError, PeerInfo, PendingInsertion, PendingPeer, RoutingTable, RoutingTableStats,
    SocketAddr, SubnetMask, Timestamp, WarningType,
};

// Re-export domain services
pub use domain::{
    calculate_bucket_index, find_k_closest, is_same_subnet, sort_peers_by_distance, xor_distance,
};

// Re-export port traits
pub use ports::{
    ConfigProvider, NetworkError, NetworkSocket, NodeIdValidator, PeerDiscoveryApi, TimeSource,
};

// Re-export service
pub use service::PeerDiscoveryService;

// Re-export IPC types
pub use ipc::{
    AuthorizationRules, FullNodeListRequestPayload, IpcHandler, PeerConnectedPayload,
    PeerDisconnectedPayload, PeerDiscoveryEventPayload, PeerDiscoveryRequestPayload, PeerFilter,
    PeerListRequestPayload, PeerListResponsePayload, SecurityError, SubsystemId,
};

// Re-export adapter types
pub use adapters::{
    EventBuilder, EventHandler, InMemoryEventPublisher, NoOpEventPublisher,
    NodeIdentityVerificationResult, PeerDiscoveryEventPublisher, PeerDiscoveryEventSubscriber,
    SubscriptionError, SubscriptionFilter,
};
