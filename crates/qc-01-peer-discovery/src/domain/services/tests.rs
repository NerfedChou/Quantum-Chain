//! Tests for Domain Services - Pure functions for Kademlia operations
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 5.1 (TDD Test Specifications)

use super::*;
use crate::domain::{Distance, IpAddr, NodeId, PeerInfo, SocketAddr, SubnetMask, Timestamp};

fn make_node_id(first_byte: u8) -> NodeId {
    let mut bytes = [0u8; 32];
    bytes[0] = first_byte;
    NodeId::new(bytes)
}

fn make_peer(first_byte: u8) -> PeerInfo {
    PeerInfo::new(
        make_node_id(first_byte),
        SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080),
        Timestamp::new(1000),
    )
}

// =============================================================================
// Test Group 1: XOR Distance Calculation (SPEC-01 Section 5.1)
// =============================================================================

#[test]
fn test_xor_distance_calculation_is_symmetric() {
    let a = make_node_id(0b1010_0000);
    let b = make_node_id(0b0101_0000);

    let dist_ab = xor_distance(&a, &b);
    let dist_ba = xor_distance(&b, &a);

    assert_eq!(dist_ab, dist_ba, "XOR distance must be symmetric");
}

#[test]
fn test_xor_distance_to_self_is_max() {
    let a = make_node_id(0b1010_1010);

    let dist = xor_distance(&a, &a);

    assert_eq!(
        dist,
        Distance::new(255),
        "Distance to self should be max (255 = closest bucket)"
    );
}

#[test]
fn test_xor_distance_identifies_correct_bucket() {
    let local = NodeId::new([0u8; 32]);

    // Differ in first bit of first byte → bucket 0
    let mut remote1 = [0u8; 32];
    remote1[0] = 0b1000_0000;
    assert_eq!(
        xor_distance(&local, &NodeId::new(remote1)),
        Distance::new(0),
        "First bit different → bucket 0"
    );

    // Differ in second bit of first byte → bucket 1
    let mut remote2 = [0u8; 32];
    remote2[0] = 0b0100_0000;
    assert_eq!(
        xor_distance(&local, &NodeId::new(remote2)),
        Distance::new(1),
        "Second bit different → bucket 1"
    );

    // Differ in first bit of second byte → bucket 8
    let mut remote3 = [0u8; 32];
    remote3[1] = 0b1000_0000;
    assert_eq!(
        xor_distance(&local, &NodeId::new(remote3)),
        Distance::new(8),
        "First bit of second byte → bucket 8"
    );
}

#[test]
fn test_xor_distance_ordering_for_closest_peers() {
    let target = NodeId::new([0u8; 32]);

    // Create peers at different distances
    let mut _far = [0u8; 32];
    _far[0] = 0b1000_0000; // Bucket 0 (farthest)
    let peer_far = make_peer(0b1000_0000);

    let mut mid = [0u8; 32];
    mid[1] = 0b1000_0000; // Bucket 8 (middle)
    let mut peer_mid = make_peer(0);
    peer_mid.node_id = NodeId::new(mid);

    let mut close = [0u8; 32];
    close[31] = 0b0000_0001; // Bucket 255 (closest)
    let mut peer_close = make_peer(0);
    peer_close.node_id = NodeId::new(close);

    let peers = vec![peer_far.clone(), peer_mid.clone(), peer_close.clone()];
    let sorted = sort_peers_by_distance(&peers, &target);

    // XOR metric: higher bucket index = closer = sorted first
    assert_eq!(
        sorted[0].node_id, peer_close.node_id,
        "Closest peer (highest bucket index) first"
    );
    assert_eq!(
        sorted[1].node_id, peer_mid.node_id,
        "Middle distance peer second"
    );
    assert_eq!(
        sorted[2].node_id, peer_far.node_id,
        "Farthest peer (lowest bucket index) last"
    );
}

// =============================================================================
// Test Group 3: IP Diversity
// Reference: SPEC-01 Section 5.1 (Sybil Attack Resistance Tests)
// =============================================================================

#[test]
fn test_same_subnet_ipv4() {
    let a = IpAddr::v4(192, 168, 1, 100);
    let b = IpAddr::v4(192, 168, 1, 200);
    let c = IpAddr::v4(192, 168, 2, 100);
    let mask = SubnetMask::new(24);

    assert!(
        is_same_subnet(&a, &b, &mask),
        "192.168.1.x should be same /24"
    );
    assert!(
        !is_same_subnet(&a, &c, &mask),
        "192.168.1.x and 192.168.2.x should be different /24"
    );
}

#[test]
fn test_different_subnets_ipv4() {
    let a = IpAddr::v4(10, 0, 0, 1);
    let b = IpAddr::v4(10, 0, 1, 1);
    let mask = SubnetMask::new(24);

    assert!(
        !is_same_subnet(&a, &b, &mask),
        "10.0.0.x and 10.0.1.x should be different /24"
    );
}

#[test]
fn test_subnet_check_works_for_ipv6() {
    let mut a_bytes = [0u8; 16];
    let mut b_bytes = [0u8; 16];
    let mut c_bytes = [0u8; 16];

    // Same /48 prefix
    a_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3]);
    a_bytes[6] = 0x00;
    b_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa3]);
    b_bytes[6] = 0xFF;

    // Different /48 prefix
    c_bytes[0..6].copy_from_slice(&[0x20, 0x01, 0x0d, 0xb8, 0x85, 0xa4]);

    let a = IpAddr::v6(a_bytes);
    let b = IpAddr::v6(b_bytes);
    let c = IpAddr::v6(c_bytes);
    let mask = SubnetMask::new(48);

    assert!(is_same_subnet(&a, &b, &mask), "Same /48 should match");
    assert!(
        !is_same_subnet(&a, &c, &mask),
        "Different /48 should not match"
    );
}

#[test]
fn test_ipv4_ipv6_never_same_subnet() {
    let v4 = IpAddr::v4(192, 168, 1, 1);
    let v6 = IpAddr::v6([0u8; 16]);
    let mask = SubnetMask::new(0); // Even with /0 mask

    assert!(
        !is_same_subnet(&v4, &v6, &mask),
        "IPv4 and IPv6 are never in same subnet"
    );
}

// =============================================================================
// Test: find_k_closest
// =============================================================================

#[test]
fn test_find_k_closest_returns_correct_count() {
    let target = NodeId::new([0u8; 32]);

    // Create 10 peers
    let peers: Vec<PeerInfo> = (1..=10).map(|i| make_peer(i as u8)).collect();

    let closest_3 = find_k_closest(&peers, &target, 3);
    assert_eq!(closest_3.len(), 3, "Should return exactly k peers");

    let closest_20 = find_k_closest(&peers, &target, 20);
    assert_eq!(closest_20.len(), 10, "Should return all peers if k > len");
}
