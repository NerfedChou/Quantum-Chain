//! # Peer Discovery & Routing Subsystem
//!
//! **Subsystem ID:** 1  
//! **Specification:** `SPECS/SPEC-01-PEER-DISCOVERY.md` v2.4
//!
//! This crate implements the Kademlia Distributed Hash Table (DHT) for
//! peer discovery and routing in the Quantum-Chain network.
//!
//! ## Zero-Dependency Core
//!
//! The core library (domain, ports, service) has **ZERO external dependencies**.
//! All adapters and integrations are feature-gated for true plug-and-play:
//!
//! - `ipc` - Event bus integration (shared-types)
//! - `rpc` - API Gateway (serde, serde_json)
//! - `bootstrap` - Bootstrap handler (uuid)
//! - `network` - UDP/TOML adapters (tokio, toml)
//!
//! ## Architecture
//!
//! The crate follows Hexagonal Architecture with:
//! - **Domain Layer:** Pure Kademlia logic (XOR distance, k-buckets, routing table)
//! - **Ports Layer:** Trait definitions for external dependencies
//! - **Service Layer:** Wires domain to ports
//! - **Adapters Layer:** Concrete implementations (feature-gated)
//!
//! ## Example
//!
//! ```rust
//! use qc_01_peer_discovery::{
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

// =============================================================================
// CORE MODULES (Zero Dependencies)
// =============================================================================

pub mod domain;
pub mod ports;
pub mod service;

// =============================================================================
// FEATURE-GATED MODULES
// =============================================================================

/// Transport layer (QUIC, etc.)
pub mod transport;

/// IPC module for event bus integration.
/// Requires feature: `ipc`
#[cfg(feature = "ipc")]
pub mod ipc;

/// Adapters for external integrations.
/// Different adapters require different features.
#[cfg(any(
    feature = "ipc",
    feature = "rpc",
    feature = "bootstrap",
    feature = "network"
))]
pub mod adapters;

/// Test utilities (FixedTimeSource, etc.)
/// Requires feature: `test-utils`
#[cfg(feature = "test-utils")]
pub mod test_utils;

// =============================================================================
// CORE RE-EXPORTS (Always Available)
// =============================================================================

// Domain entities
pub use domain::{
    BanReason, DisconnectReason, Distance, IpAddr, KBucket, KademliaConfig, NodeId,
    PeerDiscoveryError, PeerInfo, PendingInsertion, PendingPeer, RoutingTable, RoutingTableStats,
    SocketAddr, SubnetMask, Timestamp, WarningType,
};

// Domain services
pub use domain::{
    bucket_for_peer, calculate_bucket_index, find_k_closest, is_same_subnet, sort_peers_by_distance, xor_distance,
};

// Advanced Peer Discovery (Phase 1-3)
pub use domain::{
    // Phase 1: Anti-Eclipse Hardening
    AddressManager, AddressManagerConfig, AddressManagerError, AddressManagerStats,
    PeerScore, PeerScoreConfig, PeerScoreManager,
    // Phase 2: Network Health
    AcceptResult, ConnectionDirection, ConnectionInfo, ConnectionSlots, ConnectionSlotsConfig, ConnectionStats,
    BucketFreshness, FeelerConfig, FeelerProbe, FeelerResult, FeelerState,
    ForkId, HandshakeConfig, HandshakeData, HandshakeResult, PeerClassification, RejectReason, verify_handshake,
    // Phase 3: Enhanced Identity
    Capability, CapabilityData, CapabilityType, EnrCache, EnrConfig, NodeRecord, PublicKey, Signature,
};

// Port traits
pub use ports::{
    ConfigProvider, NetworkError, NetworkSocket, NodeIdValidator, PeerDiscoveryApi, TimeSource,
    VerificationHandler,
};

// Service
pub use service::PeerDiscoveryService;

// =============================================================================
// IPC RE-EXPORTS (Requires `ipc` feature)
// =============================================================================

#[cfg(feature = "ipc")]
pub use ipc::{
    AuthorizationRules, FullNodeListRequestPayload, IpcHandler, PeerConnectedPayload,
    PeerDisconnectedPayload, PeerDiscoveryEventPayload, PeerDiscoveryRequestPayload, PeerFilter,
    PeerListRequestPayload, PeerListResponsePayload, SecurityError, SubsystemId,
    BootstrapRequest, BootstrapResult, VerifyNodeIdentityRequest,
};

// =============================================================================
// ADAPTER RE-EXPORTS (Feature-Gated)
// =============================================================================

// Network adapters - pure types always available when adapters module exists
#[cfg(any(feature = "ipc", feature = "rpc", feature = "bootstrap", feature = "network"))]
pub use adapters::{
    NoOpNetworkSocket, NoOpNodeIdValidator, ProofOfWorkValidator, 
    StaticConfigProvider, SystemTimeSource,
};

// IPC/EDA adapters (publisher, subscriber)
#[cfg(feature = "ipc")]
pub use adapters::{
    EventBuilder, EventHandler, InMemoryEventPublisher, NoOpEventPublisher,
    NodeIdentityVerificationResult, PeerDiscoveryEventPublisher, PeerDiscoveryEventSubscriber,
    SubscriptionError, SubscriptionFilter, VerificationOutcome,
    InMemoryVerificationPublisher, NoOpVerificationPublisher, VerificationRequestPublisher,
};

// RPC adapters (serde-based)
#[cfg(feature = "rpc")]
pub use adapters::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc01Metrics, RpcNetworkInfo,
    RpcNodeInfo, RpcPeerInfo, RpcPorts, RpcProtocols,
};

// Bootstrap handler
#[cfg(feature = "bootstrap")]
pub use adapters::BootstrapHandler;

// Network adapters (tokio-based)
#[cfg(feature = "network")]
pub use adapters::{ConfigError, MessageType, TomlConfigProvider, UdpNetworkSocket};

// =============================================================================
// TEST UTILITIES (Requires `test-utils` feature)
// =============================================================================

#[cfg(feature = "test-utils")]
pub use test_utils::FixedTimeSource;

