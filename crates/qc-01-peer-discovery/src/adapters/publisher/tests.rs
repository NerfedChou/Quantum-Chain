//! Tests for Publisher Adapter
use super::*;
use crate::domain::{
    BanReason, DisconnectReason, IpAddr, NodeId, PeerInfo, SocketAddr, Timestamp, WarningType,
};
use crate::ipc::payloads::PeerDiscoveryEventPayload;

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
fn test_event_builder_peer_banned() {
    let builder = EventBuilder::new();
    let node_id = NodeId::new([1u8; 32]);
    let event = builder.peer_banned(node_id, BanReason::MalformedMessage, 3600);

    match event {
        PeerDiscoveryEventPayload::PeerBanned(payload) => {
            assert_eq!(payload.node_id, node_id);
            assert_eq!(payload.reason, BanReason::MalformedMessage);
            assert_eq!(payload.duration_seconds, 3600);
        }
        _ => panic!("Expected PeerBanned event"),
    }
}

#[test]
fn test_event_builder_bootstrap_completed() {
    let builder = EventBuilder::new();
    let event = builder.bootstrap_completed(50, 5000);

    match event {
        PeerDiscoveryEventPayload::BootstrapCompleted(payload) => {
            assert_eq!(payload.peer_count, 50);
            assert_eq!(payload.duration_ms, 5000);
        }
        _ => panic!("Expected BootstrapCompleted event"),
    }
}

#[test]
fn test_event_builder_routing_table_warning() {
    let builder = EventBuilder::new();
    let event = builder.routing_table_warning(
        WarningType::TooFewPeers,
        "Only 5 peers available".to_string(),
    );

    match event {
        PeerDiscoveryEventPayload::RoutingTableWarning(payload) => {
            assert_eq!(payload.warning_type, WarningType::TooFewPeers);
            assert_eq!(payload.details, "Only 5 peers available");
        }
        _ => panic!("Expected RoutingTableWarning event"),
    }
}

#[test]
fn test_noop_publisher() {
    let publisher = NoOpEventPublisher::new();
    let builder = EventBuilder::new();

    let event = builder.bootstrap_completed(10, 1000);
    assert!(publisher.publish(event).is_ok());
    assert_eq!(publisher.get_event_count(), 1);

    let event2 = builder.peer_disconnected(NodeId::new([1u8; 32]), DisconnectReason::Timeout);
    assert!(publisher.publish(event2).is_ok());
    assert_eq!(publisher.get_event_count(), 2);
}

#[test]
fn test_in_memory_publisher() {
    let publisher = InMemoryEventPublisher::new();
    let builder = EventBuilder::new();

    let event = builder.bootstrap_completed(10, 1000);
    assert!(publisher.publish(event).is_ok());

    let events = publisher.get_events();
    assert_eq!(events.len(), 1);

    match &events[0] {
        PeerDiscoveryEventPayload::BootstrapCompleted(payload) => {
            assert_eq!(payload.peer_count, 10);
        }
        _ => panic!("Expected BootstrapCompleted"),
    }

    publisher.clear();
    assert_eq!(publisher.get_events().len(), 0);
}
