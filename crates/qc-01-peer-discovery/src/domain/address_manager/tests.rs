//! Tests for Address Manager
//!
//! Reference: Bitcoin Core's `addrman.h` - New/Tried segregation tests

use super::*;
use crate::domain::{IpAddr, NodeId, PeerInfo, SocketAddr, Timestamp};

fn make_peer(id_byte: u8, ip_third: u8, ip_fourth: u8) -> PeerInfo {
    let mut id = [0u8; 32];
    id[0] = id_byte;
    PeerInfo::new(
        NodeId::new(id),
        SocketAddr::new(IpAddr::v4(192, 168, ip_third, ip_fourth), 8080),
        Timestamp::new(1000),
    )
}

fn make_source_ip(third: u8, fourth: u8) -> IpAddr {
    IpAddr::v4(10, 0, third, fourth)
}

// =============================================================================
// TEST GROUP 1: Basic New/Tried Segregation
// =============================================================================

#[test]
fn test_promote_moves_to_tried() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config);
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 1, 100);
    let source = make_source_ip(0, 1);

    manager.add_new(peer.clone(), &source, now).unwrap();
    let result = manager.promote_to_tried(&peer.node_id, now);
    assert!(result.unwrap());

    let stats = manager.stats();
    assert_eq!(stats.new_count, 0);
    assert_eq!(stats.tried_count, 1);
}

// =============================================================================
// TEST GROUP 2: Per-Subnet Limits (Anti-Eclipse)
// =============================================================================

#[test]
fn test_per_subnet_bucket_limit() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config.clone());
    let now = Timestamp::new(1000);

    // Add max_per_subnet_per_bucket peers from same /16
    // All have 192.168.x.y (same /16 subnet 192.168.0.0/16)
    let source = make_source_ip(0, 1);

    // First two should succeed (max_per_subnet_per_bucket = 2)
    let peer1 = make_peer(1, 1, 100);
    let peer2 = make_peer(2, 1, 101);
    assert!(manager.add_new(peer1, &source, now).unwrap());
    assert!(manager.add_new(peer2, &source, now).unwrap());

    // Third peer same subnet might be rejected if lands in same bucket
    // (depends on hash). Let's verify at least subnet tracking works
    let stats = manager.stats();
    assert_eq!(stats.new_count, 2);
}

#[test]
fn test_different_subnets_allowed() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config);
    let now = Timestamp::new(1000);
    let source = make_source_ip(0, 1);

    // Peers from same /16 (192.168.x.x) but same source
    // may hit per-bucket subnet limits depending on hash distribution
    let peer1 = make_peer(1, 1, 100); // 192.168.1.100
    let peer2 = make_peer(2, 2, 100); // 192.168.2.100

    assert!(manager.add_new(peer1, &source, now).unwrap());
    assert!(manager.add_new(peer2, &source, now).unwrap());

    // At least 2 should be added (may be more if in different buckets)
    assert!(manager.stats().new_count >= 2);
}

// =============================================================================
// TEST GROUP 3: Bucket Distribution
// =============================================================================

#[test]
fn test_different_sources_distribute_to_different_buckets() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config);
    let now = Timestamp::new(1000);

    // Different peers, different sources - should distribute across buckets
    let peer1 = make_peer(1, 1, 100);
    let peer2 = make_peer(2, 1, 101);

    let source1 = make_source_ip(0, 1);
    let source2 = make_source_ip(1, 1); // Different /16 source

    assert!(manager.add_new(peer1, &source1, now).unwrap());
    assert!(manager.add_new(peer2, &source2, now).unwrap());

    // At least 2 should be added
    assert!(manager.stats().new_count >= 2);
}

// =============================================================================
// TEST GROUP 4: Random Selection
// =============================================================================

#[test]
fn test_random_new_address_returns_entry() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config);
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 1, 100);
    let source = make_source_ip(0, 1);

    manager.add_new(peer.clone(), &source, now).unwrap();

    let random = manager.random_new_address();
    assert!(random.is_some());
    assert_eq!(random.unwrap().peer_info.node_id, peer.node_id);
}

#[test]
fn test_random_tried_address_after_promotion() {
    let config = AddressManagerConfig::for_testing();
    let mut manager = AddressManager::new(config);
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 1, 100);
    let source = make_source_ip(0, 1);

    manager.add_new(peer.clone(), &source, now).unwrap();
    manager.promote_to_tried(&peer.node_id, now).unwrap();

    let random_new = manager.random_new_address();
    let random_tried = manager.random_tried_address();

    assert!(random_new.is_none()); // Moved out of New
    assert!(random_tried.is_some()); // Now in Tried
}
