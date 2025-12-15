use serde::{Deserialize, Serialize};

/// Peer info formatted for JSON-RPC responses (matches Ethereum admin_peers format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPeerInfo {
    /// Peer's node ID as hex string
    pub id: String,
    /// Peer's name/client info (placeholder for now)
    pub name: String,
    /// Enode URL format
    pub enode: String,
    /// Remote address as "ip:port"
    #[serde(rename = "remoteAddress")]
    pub remote_address: String,
    /// Capabilities (placeholder)
    pub caps: Vec<String>,
    /// Network info
    pub network: RpcNetworkInfo,
}

/// Network info for a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNetworkInfo {
    /// Local address of our node.
    #[serde(rename = "localAddress")]
    pub local_address: String,
    /// Remote address of the peer.
    #[serde(rename = "remoteAddress")]
    pub remote_address: String,
    /// Whether this is an inbound connection.
    pub inbound: bool,
    /// Whether this peer is trusted.
    pub trusted: bool,
    /// Whether this is a static node (manually configured).
    #[serde(rename = "static")]
    pub static_node: bool,
}

/// Node info for admin_nodeInfo response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNodeInfo {
    /// Node ID as hex string
    pub id: String,
    /// Node name
    pub name: String,
    /// Enode URL
    pub enode: String,
    /// IP address
    pub ip: String,
    /// Ports
    pub ports: RpcPorts,
    /// Listen address
    #[serde(rename = "listenAddr")]
    pub listen_addr: String,
    /// Protocols
    pub protocols: RpcProtocols,
}

/// Port info for node discovery and listening.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPorts {
    /// UDP port for peer discovery protocol.
    pub discovery: u16,
    /// TCP port for P2P listener.
    pub listener: u16,
}

/// Protocol info for supported network protocols.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcProtocols {
    /// Ethereum protocol information.
    pub eth: RpcEthProtocol,
}

/// Ethereum protocol info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEthProtocol {
    /// Network ID (e.g., 1 for mainnet).
    pub network: u64,
    /// Current total difficulty.
    pub difficulty: u64,
    /// Genesis block hash.
    pub genesis: String,
    /// Current head block hash.
    pub head: String,
}

/// Subsystem-specific metrics for qc-01.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qc01Metrics {
    /// Total peers in routing table
    pub total_peers: usize,
    /// Maximum peers allowed
    pub max_peers: usize,
    /// Number of k-buckets with peers
    pub buckets_used: usize,
    /// Maximum buckets
    pub max_buckets: usize,
    /// Number of banned peers
    pub banned_count: usize,
    /// Peers pending verification
    pub pending_verification_count: usize,
    /// Maximum pending peers
    pub max_pending_peers: usize,
    /// Age of oldest peer in seconds
    pub oldest_peer_age_seconds: u64,
}

/// Error type for API query responses (matches shared-bus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiQueryError {
    /// JSON-RPC error code.
    pub code: i32,
    /// Human-readable error message.
    pub message: String,
}
