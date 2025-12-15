//! Tests for API Handler Adapter
use super::*;
use crate::domain::{
    BanDetails, IpAddr, KademliaConfig, NodeId, PeerDiscoveryError, PeerInfo, RoutingTable,
    RoutingTableStats, SocketAddr, Timestamp,
};
use crate::ports::PeerDiscoveryApi;

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

    /// Adds a single test peer to the routing table.
    fn add_test_peer(&mut self, index: u8, now: Timestamp) {
        let mut id_bytes = [0u8; 32];
        id_bytes[0] = index;
        let peer = PeerInfo::new(
            NodeId::new(id_bytes),
            SocketAddr::new(IpAddr::v4(192, 168, 1, index), 30303),
            now,
        );
        if self.table.stage_peer(peer.clone(), now) == Ok(true) {
            let _ = self.table.on_verification_result(&peer.node_id, true, now);
        }
    }

    fn with_peers(count: usize) -> Self {
        let mut service = Self::new();
        let now = Timestamp::new(1000);
        for i in 1..=count {
            service.add_test_peer(i as u8, now);
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

    fn ban_peer(&mut self, node_id: NodeId, details: BanDetails) -> Result<(), PeerDiscoveryError> {
        self.table.ban_peer(node_id, details, Timestamp::new(1000))
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
