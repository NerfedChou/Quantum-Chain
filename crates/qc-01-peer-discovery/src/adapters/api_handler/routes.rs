use super::types::*;
use crate::domain::{NodeId, PeerInfo};
use crate::ports::PeerDiscoveryApi;

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
