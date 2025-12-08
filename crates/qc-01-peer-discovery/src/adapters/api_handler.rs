//! # API Gateway Request Handler
//!
//! Handles requests from qc-16 (API Gateway) for admin_peers, admin_nodeInfo, etc.
//!
//! ## Supported Methods
//!
//! - `get_peers` - Returns connected peers for admin_peers RPC
//! - `get_node_info` - Returns node info for admin_nodeInfo RPC
//! - `add_peer` - Adds a peer (admin_addPeer)
//! - `remove_peer` - Removes a peer (admin_removePeer)
//! - `get_subsystem_metrics` - Returns qc-01 specific metrics for debug panel
//! - `ping` - Health check

use crate::domain::{NodeId, PeerInfo};
use crate::ports::PeerDiscoveryApi;
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
    #[serde(rename = "localAddress")]
    pub local_address: String,
    #[serde(rename = "remoteAddress")]
    pub remote_address: String,
    pub inbound: bool,
    pub trusted: bool,
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

/// Port info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPorts {
    pub discovery: u16,
    pub listener: u16,
}

/// Protocol info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcProtocols {
    pub eth: RpcEthProtocol,
}

/// Ethereum protocol info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEthProtocol {
    pub network: u64,
    pub difficulty: u64,
    pub genesis: String,
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

/// API Gateway request handler for qc-01.
pub struct ApiGatewayHandler<S> {
    service: S,
    local_node_id: NodeId,
    listen_port: u16,
}

impl<S: PeerDiscoveryApi> ApiGatewayHandler<S> {
    /// Create a new API handler.
    pub fn new(service: S, local_node_id: NodeId, listen_port: u16) -> Self {
        Self {
            service,
            local_node_id,
            listen_port,
        }
    }

    /// Get mutable access to the service.
    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    /// Handle get_peers request (admin_peers).
    ///
    /// Returns up to 100 connected peers in Ethereum-compatible format.
    pub fn handle_get_peers(&self) -> serde_json::Value {
        let peers = self.service.get_random_peers(100);
        let rpc_peers: Vec<RpcPeerInfo> = peers.iter().map(|p| self.peer_to_rpc(p)).collect();
        serde_json::to_value(rpc_peers).unwrap_or_default()
    }

    /// Handle get_node_info request (admin_nodeInfo).
    pub fn handle_get_node_info(&self) -> serde_json::Value {
        let node_id_hex = encode_hex(self.local_node_id.as_bytes());
        let enode = format!("enode://{}@0.0.0.0:{}", node_id_hex, self.listen_port);

        let info = RpcNodeInfo {
            id: node_id_hex.clone(),
            name: "Quantum-Chain/v0.1.0".to_string(),
            enode,
            ip: "0.0.0.0".to_string(),
            ports: RpcPorts {
                discovery: self.listen_port,
                listener: self.listen_port,
            },
            listen_addr: format!("0.0.0.0:{}", self.listen_port),
            protocols: RpcProtocols {
                eth: RpcEthProtocol {
                    network: 1,
                    difficulty: 0,
                    genesis: "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                    head: "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                },
            },
        };

        serde_json::to_value(info).unwrap_or_default()
    }

    /// Handle get_subsystem_metrics request (debug panel).
    pub fn handle_get_metrics(&self) -> serde_json::Value {
        let stats = self.service.get_stats();

        let metrics = Qc01Metrics {
            total_peers: stats.total_peers,
            max_peers: stats.max_pending_peers, // Use max_pending_peers as proxy for max_peers
            buckets_used: stats.buckets_used,
            max_buckets: 256, // Standard Kademlia
            banned_count: stats.banned_count,
            pending_verification_count: stats.pending_verification_count,
            max_pending_peers: stats.max_pending_peers,
            oldest_peer_age_seconds: stats.oldest_peer_age_seconds,
        };

        serde_json::to_value(metrics).unwrap_or_default()
    }

    /// Handle ping request (health check).
    pub fn handle_ping(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "ok",
            "subsystem": "qc-01-peer-discovery"
        })
    }

    /// Convert internal PeerInfo to RPC format.
    fn peer_to_rpc(&self, peer: &PeerInfo) -> RpcPeerInfo {
        let node_id_hex = encode_hex(peer.node_id.as_bytes());
        let addr = format_socket_addr(&peer.socket_addr);
        let enode = format!("enode://{}@{}", node_id_hex, addr);

        RpcPeerInfo {
            id: node_id_hex,
            name: "Quantum-Chain/v0.1.0".to_string(),
            enode,
            remote_address: addr.clone(),
            caps: vec!["eth/68".to_string()],
            network: RpcNetworkInfo {
                local_address: format!("0.0.0.0:{}", self.listen_port),
                remote_address: addr,
                inbound: false,
                trusted: false,
                static_node: false,
            },
        }
    }
}

/// Format a SocketAddr as "ip:port" string.
fn format_socket_addr(addr: &crate::domain::SocketAddr) -> String {
    let ip_str = match addr.ip {
        crate::domain::IpAddr::V4(bytes) => {
            format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
        }
        crate::domain::IpAddr::V6(bytes) => {
            // Simplified IPv6 formatting
            format!(
                "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}",
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15]
            )
        }
    };
    format!("{}:{}", ip_str, addr.port)
}

/// Helper to encode bytes as hex string.
fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(s, "{:02x}", b).unwrap();
    }
    s
}

/// Handle an API query from the event bus.
///
/// This function is called by the event loop when a `BlockchainEvent::ApiQuery`
/// is received targeting "qc-01-peer-discovery".
pub fn handle_api_query<S: PeerDiscoveryApi>(
    handler: &ApiGatewayHandler<S>,
    method: &str,
    _params: &serde_json::Value,
) -> Result<serde_json::Value, ApiQueryError> {
    match method {
        "get_peers" | "admin_peers" => Ok(handler.handle_get_peers()),
        "get_node_info" | "admin_nodeInfo" => Ok(handler.handle_get_node_info()),
        "get_subsystem_metrics" | "debug_subsystemMetrics" => Ok(handler.handle_get_metrics()),
        "ping" => Ok(handler.handle_ping()),
        _ => Err(ApiQueryError {
            code: -32601,
            message: format!("Method not found: {}", method),
        }),
    }
}

/// Error type for API query responses (matches shared-bus).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiQueryError {
    pub code: i32,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        BanReason, IpAddr, KademliaConfig, PeerDiscoveryError, RoutingTable, RoutingTableStats,
        SocketAddr, Timestamp,
    };

    struct TestService {
        table: RoutingTable,
    }

    impl TestService {
        fn new() -> Self {
            let local_id = NodeId::new([0u8; 32]);
            let config = KademliaConfig::for_testing();
            Self {
                table: RoutingTable::new(local_id, config),
            }
        }

        fn with_peers(count: usize) -> Self {
            let mut service = Self::new();
            let now = Timestamp::new(1000);

            for i in 1..=count {
                let mut id_bytes = [0u8; 32];
                id_bytes[0] = i as u8;
                let peer = PeerInfo::new(
                    NodeId::new(id_bytes),
                    SocketAddr::new(IpAddr::v4(192, 168, 1, i as u8), 30303),
                    now,
                );
                if let Ok(true) = service.table.stage_peer(peer.clone(), now) {
                    let _ = service
                        .table
                        .on_verification_result(&peer.node_id, true, now);
                }
            }
            service
        }
    }

    impl PeerDiscoveryApi for TestService {
        fn find_closest_peers(&self, target_id: NodeId, count: usize) -> Vec<PeerInfo> {
            self.table.find_closest_peers(&target_id, count)
        }

        fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
            self.table.stage_peer(peer, Timestamp::new(1000))
        }

        fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
            self.table.get_random_peers(count)
        }

        fn ban_peer(
            &mut self,
            node_id: NodeId,
            duration_seconds: u64,
            reason: BanReason,
        ) -> Result<(), PeerDiscoveryError> {
            self.table
                .ban_peer(node_id, duration_seconds, reason, Timestamp::new(1000))
        }

        fn is_banned(&self, node_id: NodeId) -> bool {
            self.table.is_banned(&node_id, Timestamp::new(1000))
        }

        fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
            self.table.touch_peer(&node_id, Timestamp::new(1000))
        }

        fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
            self.table.remove_peer(&node_id)
        }

        fn get_stats(&self) -> RoutingTableStats {
            self.table.stats(Timestamp::new(1000))
        }
    }

    #[test]
    fn test_handle_get_peers_empty() {
        let service = TestService::new();
        let local_id = NodeId::new([0u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        let result = handler.handle_get_peers();
        let peers: Vec<RpcPeerInfo> = serde_json::from_value(result).unwrap();
        assert!(peers.is_empty());
    }

    #[test]
    fn test_handle_get_peers_with_peers() {
        let service = TestService::with_peers(5);
        let local_id = NodeId::new([0u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        let result = handler.handle_get_peers();
        let peers: Vec<RpcPeerInfo> = serde_json::from_value(result).unwrap();
        assert_eq!(peers.len(), 5);

        // Verify peer format
        let peer = &peers[0];
        assert!(!peer.id.is_empty());
        assert!(peer.enode.starts_with("enode://"));
        assert!(peer.remote_address.contains(":30303"));
    }

    #[test]
    fn test_handle_get_node_info() {
        let service = TestService::new();
        let local_id = NodeId::new([1u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        let result = handler.handle_get_node_info();
        let info: RpcNodeInfo = serde_json::from_value(result).unwrap();

        assert!(info.enode.starts_with("enode://"));
        assert!(info.enode.contains("0101010101")); // First bytes of node ID
        assert_eq!(info.ports.listener, 30303);
    }

    #[test]
    fn test_handle_get_metrics() {
        let service = TestService::with_peers(3);
        let local_id = NodeId::new([0u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        let result = handler.handle_get_metrics();
        let metrics: Qc01Metrics = serde_json::from_value(result).unwrap();

        assert_eq!(metrics.total_peers, 3);
        assert_eq!(metrics.pending_verification_count, 0);
    }

    #[test]
    fn test_handle_ping() {
        let service = TestService::new();
        let local_id = NodeId::new([0u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        let result = handler.handle_ping();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["subsystem"], "qc-01-peer-discovery");
    }

    #[test]
    fn test_handle_api_query() {
        let service = TestService::with_peers(2);
        let local_id = NodeId::new([0u8; 32]);
        let handler = ApiGatewayHandler::new(service, local_id, 30303);

        // Test get_peers
        let result = handle_api_query(&handler, "get_peers", &serde_json::Value::Null);
        assert!(result.is_ok());

        // Test unknown method
        let result = handle_api_query(&handler, "unknown_method", &serde_json::Value::Null);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, -32601);
    }
}
