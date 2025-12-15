//! Tests for PeerDiscoveryService

use super::*;
use crate::domain::{
    BanDetails, BanReason, IpAddr, KademliaConfig, NodeId, PeerInfo, RoutingTableStats, SocketAddr,
    Timestamp,
};
use crate::ports::{PeerDiscoveryApi, TimeSource};
use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe TimeSource for tests requiring time advancement.
/// Uses AtomicU64 to allow multiple readers while supporting `advance()`.
struct ControllableTimeSource {
    time: AtomicU64,
}

impl ControllableTimeSource {
    fn new(initial: u64) -> Self {
        Self {
            time: AtomicU64::new(initial),
        }
    }

    /// Advances the internal clock by the specified seconds.
    /// Used to test timeout and expiration behaviors.
    fn advance(&self, secs: u64) {
        self.time.fetch_add(secs, Ordering::SeqCst);
    }
}

impl TimeSource for ControllableTimeSource {
    fn now(&self) -> Timestamp {
        Timestamp::new(self.time.load(Ordering::SeqCst))
    }
}

/// Creates a NodeId with first byte set to `val`, rest zeroed.
fn make_node_id(val: u8) -> NodeId {
    let mut bytes = [0u8; 32];
    bytes[0] = val;
    NodeId::new(bytes)
}

/// Creates a PeerInfo with unique NodeId and IP in 192.168.x.1 subnet.
fn make_peer(val: u8) -> PeerInfo {
    PeerInfo::new(
        make_node_id(val),
        SocketAddr::new(IpAddr::v4(192, 168, val, 1), 8080),
        Timestamp::new(1000),
    )
}

#[test]
fn test_service_add_peer_stages_for_verification() {
    let local_id = make_node_id(0);
    let config = KademliaConfig::for_testing();
    let time_source = Box::new(ControllableTimeSource::new(1000));
    let mut service = PeerDiscoveryService::new(local_id, config, time_source);

    let peer = make_peer(1);
    let result = service.add_peer(peer);

    // INVARIANT-7: New peers enter staging, not routing table
    assert!(result.is_ok());
    assert!(result.unwrap());

    let stats = service.get_stats();
    assert_eq!(stats.pending_verification_count, 1);
    assert_eq!(stats.total_peers, 0);
}

/// Helper to setup service and add a peer
fn setup_service_with_peer() -> (PeerDiscoveryService, NodeId) {
    let local_id = make_node_id(0);
    let config = KademliaConfig::for_testing();
    let time_source = Box::new(ControllableTimeSource::new(1000));
    let mut service = PeerDiscoveryService::new(local_id, config, time_source);

    let peer = make_peer(1);
    let node_id = peer.node_id;

    service.add_peer(peer).unwrap();
    (service, node_id)
}

#[test]
fn test_service_verification_promotes_peer() {
    let (mut service, node_id) = setup_service_with_peer();

    assert_eq!(service.get_stats().pending_verification_count, 1);

    // INVARIANT-7: Verified peers move from staging to routing table
    let result = service.on_verification_result(&node_id, true).unwrap();
    assert!(result.is_none());

    let stats = service.get_stats();
    assert_eq!(stats.pending_verification_count, 0);
    assert_eq!(stats.total_peers, 1);
}

#[test]
fn test_service_failed_verification_drops_peer() {
    let (mut service, node_id) = setup_service_with_peer();

    // SPEC-01 Section 2.2: Failed verification triggers silent drop, NOT ban
    let result = service.on_verification_result(&node_id, false).unwrap();
    assert!(result.is_none());

    let stats = service.get_stats();
    assert_eq!(stats.pending_verification_count, 0);
    assert_eq!(stats.total_peers, 0);
}

#[test]
fn test_service_implements_api_trait() {
    let local_id = make_node_id(0);
    let config = KademliaConfig::for_testing();
    let time_source = Box::new(ControllableTimeSource::new(1000));
    let service = PeerDiscoveryService::new(local_id, config, time_source);

    // Verify PeerDiscoveryService implements the PeerDiscoveryApi trait
    fn use_api<T: PeerDiscoveryApi>(api: &T) -> RoutingTableStats {
        api.get_stats()
    }

    let stats = use_api(&service);
    assert_eq!(stats.total_peers, 0);
}

#[test]
fn test_service_ban_and_is_banned() {
    let local_id = make_node_id(0);
    let config = KademliaConfig::for_testing();
    let time_source = Box::new(ControllableTimeSource::new(1000));
    let mut service = PeerDiscoveryService::new(local_id, config, time_source);

    let peer_id = make_node_id(1);

    assert!(!service.is_banned(peer_id));

    service
        .ban_peer(peer_id, BanDetails::new(3600, BanReason::MalformedMessage))
        .unwrap();

    assert!(service.is_banned(peer_id));
}

#[test]
fn test_service_ban_expires_after_duration() {
    let local_id = make_node_id(0);
    let config = KademliaConfig::for_testing();
    let time_source = ControllableTimeSource::new(1000);
    let time_ref = std::sync::Arc::new(time_source);
    let time_clone = std::sync::Arc::clone(&time_ref);

    /// Wrapper to share ControllableTimeSource across service and test via Arc.
    struct SharedTimeSource(std::sync::Arc<ControllableTimeSource>);
    impl TimeSource for SharedTimeSource {
        fn now(&self) -> Timestamp {
            self.0.now()
        }
    }

    let mut service =
        PeerDiscoveryService::new(local_id, config, Box::new(SharedTimeSource(time_clone)));

    let peer_id = make_node_id(1);

    // Ban for 3600 seconds
    service
        .ban_peer(peer_id, BanDetails::new(3600, BanReason::MalformedMessage))
        .unwrap();

    assert!(service.is_banned(peer_id), "Peer is banned at t=1000");

    // Advance time past ban expiration (t=1000 + 3600 + 1 = 4601)
    time_ref.advance(3601);

    assert!(!service.is_banned(peer_id), "Ban expired at t=4601");
}
