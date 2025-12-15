//! Tests for IPC Payloads

use super::*;
use super::*;
use crate::domain::{IpAddr, SocketAddr, Timestamp};

fn make_peer_info(id_byte: u8) -> PeerInfo {
    let mut id_bytes = [0u8; 32];
    id_bytes[0] = id_byte;
    PeerInfo::new(
        NodeId::new(id_bytes),
        SocketAddr::new(IpAddr::v4(192, 168, 1, id_byte), 8080),
        Timestamp::new(1000),
    )
}

#[test]
fn test_peer_list_request_new() {
    let req = PeerListRequestPayload::new(20);
    assert_eq!(req.max_peers, 20);
    assert!(req.filter.is_none());
}

#[test]
fn test_peer_list_request_with_filter() {
    let filter = PeerFilter {
        min_reputation: 50,
        exclude_subnets: vec![],
    };
    let req = PeerListRequestPayload::with_filter(10, filter);
    assert_eq!(req.max_peers, 10);
    assert!(req.filter.is_some());
    assert_eq!(req.filter.unwrap().min_reputation, 50);
}

#[test]
fn test_peer_connected_payload() {
    let peer = make_peer_info(1);
    let payload = PeerConnectedPayload {
        peer_info: peer.clone(),
        bucket_index: 5,
    };
    assert_eq!(payload.peer_info.node_id, peer.node_id);
    assert_eq!(payload.bucket_index, 5);
}

#[test]
fn test_peer_disconnected_payload() {
    let node_id = NodeId::new([1u8; 32]);
    let payload = PeerDisconnectedPayload {
        node_id,
        reason: DisconnectReason::Timeout,
    };
    assert_eq!(payload.node_id, node_id);
    assert_eq!(payload.reason, DisconnectReason::Timeout);
}

#[test]
fn test_peer_list_response_payload() {
    let peers = vec![make_peer_info(1), make_peer_info(2)];
    let payload = PeerListResponsePayload {
        peers: peers.clone(),
        total_available: 100,
    };
    assert_eq!(payload.peers.len(), 2);
    assert_eq!(payload.total_available, 100);
}

#[test]
fn test_full_node_list_request() {
    let req = FullNodeListRequestPayload::new(5);
    assert_eq!(req.max_nodes, 5);
    assert!(req.preferred_region.is_none());

    let req_with_region = FullNodeListRequestPayload::with_region(10, "us-east".to_string());
    assert_eq!(req_with_region.max_nodes, 10);
    assert_eq!(
        req_with_region.preferred_region,
        Some("us-east".to_string())
    );
}
