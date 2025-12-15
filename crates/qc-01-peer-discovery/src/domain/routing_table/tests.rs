//! Tests for Routing Table Implementation
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 5.1 (TDD Test Specifications)

use super::*;
use crate::domain::{
    calculate_bucket_index, BanReason, IpAddr, KademliaConfig, NodeId, PeerDiscoveryError,
    PeerInfo, SocketAddr, Timestamp,
};

fn make_node_id(val: u8) -> NodeId {
    let mut bytes = [0u8; 32];
    bytes[0] = val;
    NodeId::new(bytes)
}

fn make_peer(val: u8, port: u16) -> PeerInfo {
    PeerInfo::new(
        make_node_id(val),
        SocketAddr::new(IpAddr::v4(192, 168, 1, val), port),
        Timestamp::new(1000),
    )
}

// =============================================================================
// Test Group 2: K-Bucket Management
// Reference: SPEC-01 Section 5.1 (TDD Test Specifications)
// =============================================================================

#[test]
fn test_bucket_rejects_when_full() {
    let mut bucket = KBucket::new();
    let k = 3;

    for i in 0..k {
        bucket.add_peer(make_peer(i as u8, 8080), Timestamp::new(1000));
    }

    assert!(bucket.is_full(k));
    assert_eq!(bucket.len(), k);
}

#[test]
fn test_bucket_rejects_21st_peer_when_full_and_all_alive() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    // NodeId with first bit = 1 maps to bucket 0 (XOR distance from local_id = 0)
    let make_bucket0_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
            Timestamp::new(1000),
        )
    };

    // Fill bucket to capacity (k=3 for testing config)
    for i in 0..table.config().k {
        let peer = make_bucket0_peer(i as u8);
        table.stage_peer(peer.clone(), now).unwrap();
        let result = table
            .on_verification_result(&peer.node_id, true, now)
            .unwrap();
        // INVARIANT-1: First k-1 peers added directly without challenge
        if i < table.config().k - 1 {
            assert!(result.is_none(), "Peer {} added directly", i);
        }
    }

    // INVARIANT-10: Additional peer triggers eviction challenge
    let extra_peer = make_bucket0_peer(100);
    table.stage_peer(extra_peer.clone(), now).unwrap();
    let result = table
        .on_verification_result(&extra_peer.node_id, true, now)
        .unwrap();

    assert!(
        result.is_some(),
        "Full bucket returns challenged peer NodeId"
    );
}

#[test]
fn test_bucket_prefers_stable_peers_over_new_peers() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    // Create a peer that goes to bucket 0
    let make_bucket0_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
            Timestamp::new(1000),
        )
    };

    // Fill bucket
    let mut peers = Vec::new();
    for i in 0..table.config().k {
        let peer = make_bucket0_peer(i as u8);
        peers.push(peer.node_id);
        table.stage_peer(peer.clone(), now).unwrap();
        table
            .on_verification_result(&peer.node_id, true, now)
            .unwrap();
    }

    // Add new peer - triggers challenge
    let new_peer = make_bucket0_peer(100);
    table.stage_peer(new_peer.clone(), now).unwrap();
    let challenged = table
        .on_verification_result(&new_peer.node_id, true, now)
        .unwrap()
        .expect("Should have challenged peer");

    // Simulate: oldest peer is ALIVE (responded to PING)
    table.on_challenge_response(&challenged, true, now).unwrap();

    // Verify: oldest peer is still in bucket, new peer rejected
    let bucket_idx = calculate_bucket_index(&local_id, &peers[0]);
    let bucket = table.get_bucket(bucket_idx).unwrap();

    assert!(
        bucket.contains(&challenged),
        "Stable peer retained per INVARIANT-10"
    );
    assert!(
        !bucket.contains(&new_peer.node_id),
        "Candidate rejected when challenged peer alive"
    );
    assert_eq!(bucket.len(), table.config().k, "Bucket maintains k peers");
}

#[test]
fn test_bucket_evicts_dead_peers_for_new_peers() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let make_bucket0_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
            Timestamp::new(1000),
        )
    };

    let mut peers = Vec::new();
    for i in 0..table.config().k {
        let peer = make_bucket0_peer(i as u8);
        peers.push(peer.node_id);
        table.stage_peer(peer.clone(), now).unwrap();
        table
            .on_verification_result(&peer.node_id, true, now)
            .unwrap();
    }

    let oldest_peer = peers[0];

    // INVARIANT-10: Full bucket triggers challenge to oldest peer
    let new_peer = make_bucket0_peer(100);
    table.stage_peer(new_peer.clone(), now).unwrap();
    let challenged = table
        .on_verification_result(&new_peer.node_id, true, now)
        .unwrap()
        .expect("Full bucket returns challenged NodeId");

    assert_eq!(challenged, oldest_peer, "Challenge targets oldest peer");

    // Simulate PONG timeout (peer is dead)
    table
        .on_challenge_response(&challenged, false, now)
        .unwrap();

    let bucket_idx = calculate_bucket_index(&local_id, &oldest_peer);
    let bucket = table.get_bucket(bucket_idx).unwrap();

    assert!(
        !bucket.contains(&oldest_peer),
        "Dead peer evicted per INVARIANT-10"
    );
    assert!(
        bucket.contains(&new_peer.node_id),
        "Candidate inserted after eviction"
    );
    assert_eq!(bucket.len(), table.config().k, "Bucket maintains k peers");
}

#[test]
fn test_bucket_challenge_in_progress_rejects_additional_candidates() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let make_bucket0_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
            Timestamp::new(1000),
        )
    };

    // Fill bucket to capacity
    for i in 0..table.config().k {
        let peer = make_bucket0_peer(i as u8);
        table.stage_peer(peer.clone(), now).unwrap();
        table
            .on_verification_result(&peer.node_id, true, now)
            .unwrap();
    }

    // First candidate triggers challenge against oldest peer
    let peer_a = make_bucket0_peer(100);
    table.stage_peer(peer_a.clone(), now).unwrap();
    let _challenged = table
        .on_verification_result(&peer_a.node_id, true, now)
        .unwrap();

    // INVARIANT-10: Only ONE pending_insertion per bucket allowed
    let peer_b = make_bucket0_peer(101);
    table.stage_peer(peer_b.clone(), now).unwrap();
    let result = table.on_verification_result(&peer_b.node_id, true, now);

    assert!(
        matches!(result, Err(PeerDiscoveryError::ChallengeInProgress)),
        "Concurrent challenge rejected per INVARIANT-10"
    );
}

// =============================================================================
// Test Group 5: Ban System
// Reference: SPEC-01 Section 5.1 (TDD Test Specifications)
// =============================================================================

#[test]
fn test_bucket_rejects_peer_if_banned() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);

    table
        .ban_peer(peer.node_id, BanDetails::new(60, BanReason::ManualBan), now)
        .unwrap();

    // INVARIANT-4: Banned peers excluded from routing table
    let result = table.stage_peer(peer, now);

    assert!(
        matches!(result, Err(PeerDiscoveryError::PeerBanned)),
        "Banned peer rejected per INVARIANT-4"
    );
}

#[test]
fn test_banned_peer_expires_after_duration() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);

    table
        .ban_peer(peer.node_id, BanDetails::new(60, BanReason::ManualBan), now)
        .unwrap();

    // Ban active at t=1000 and t=1059 (59 seconds elapsed)
    assert!(table.is_banned(&peer.node_id, now));
    assert!(table.is_banned(&peer.node_id, Timestamp::new(1059)));

    // Ban expired at t=1061 (61 seconds elapsed > 60 second ban)
    assert!(!table.is_banned(&peer.node_id, Timestamp::new(1061)));
}

#[test]
fn test_cannot_add_banned_peer_to_routing_table() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);

    table
        .ban_peer(peer.node_id, BanDetails::new(60, BanReason::ManualBan), now)
        .unwrap();

    let result = table.stage_peer(peer, now);

    assert!(matches!(result, Err(PeerDiscoveryError::PeerBanned)));
}

// =============================================================================
// Test Group 6: Pending Verification Staging
// Reference: SPEC-01 Section 5.1 (DDoS Edge Defense Tests)
// =============================================================================

#[test]
fn test_new_peer_goes_to_pending_verification_first() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);
    let peer_id = peer.node_id;

    table.stage_peer(peer, now).unwrap();

    // INVARIANT-7: Peer in staging area, not routing table
    assert_eq!(table.pending_verification_count(), 1);
    assert_eq!(table.total_peer_count(), 0);

    // Verification promotes peer to routing table
    table.on_verification_result(&peer_id, true, now).unwrap();

    assert_eq!(table.pending_verification_count(), 0);
    assert_eq!(table.total_peer_count(), 1);
}

#[test]
fn test_peer_silently_dropped_on_identity_valid_false() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);
    let peer_id = peer.node_id;

    table.stage_peer(peer, now).unwrap();
    assert_eq!(table.pending_verification_count(), 1);

    // SPEC-01 Section 2.2: Silent drop on verification failure (IP spoofing defense)
    table.on_verification_result(&peer_id, false, now).unwrap();

    assert_eq!(table.pending_verification_count(), 0);
    assert_eq!(table.total_peer_count(), 0);
    assert!(
        !table.is_banned(&peer_id, now),
        "Silent drop, NOT ban per BanReason security note"
    );
}

#[test]
fn test_pending_peer_times_out_after_deadline() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);

    table.stage_peer(peer, now).unwrap();
    assert_eq!(table.pending_verification_count(), 1);

    // INVARIANT-8: Peer remains until deadline
    let later = Timestamp::new(1000 + table.config().verification_timeout_secs - 1);
    table.gc_expired(later);
    assert_eq!(table.pending_verification_count(), 1);

    // INVARIANT-8: Peer removed after deadline
    let expired = Timestamp::new(1000 + table.config().verification_timeout_secs + 1);
    let removed = table.gc_expired(expired);
    assert_eq!(removed, 1);
    assert_eq!(table.pending_verification_count(), 0);
}

// =============================================================================
// Test Group 7: Bounded Staging (Memory Bomb Defense)
// Reference: SPEC-01 Section 5.1 (V2.3 Memory Bomb Defense Tests)
// =============================================================================

#[test]
fn test_staging_area_rejects_peer_when_at_capacity() {
    let local_id = make_node_id(0);
    let mut config = KademliaConfig::for_testing();
    config.max_pending_peers = 3;
    let mut table = RoutingTable::new(local_id, config);
    let now = Timestamp::new(1000);

    for i in 1..=3 {
        let peer = make_peer(i, 8080);
        table.stage_peer(peer, now).unwrap();
    }

    assert_eq!(table.pending_verification_count(), 3);

    // INVARIANT-9: Tail Drop when staging at capacity
    let extra_peer = make_peer(100, 8080);
    let result = table.stage_peer(extra_peer, now);

    assert!(
        matches!(result, Err(PeerDiscoveryError::StagingAreaFull)),
        "Staging full returns StagingAreaFull error"
    );
    assert_eq!(
        table.pending_verification_count(),
        3,
        "Staging count unchanged after rejection"
    );
}

#[test]
fn test_staging_area_uses_tail_drop_not_eviction() {
    let local_id = make_node_id(0);
    let mut config = KademliaConfig::for_testing();
    config.max_pending_peers = 2;
    let mut table = RoutingTable::new(local_id, config);
    let now = Timestamp::new(1000);

    let peer1 = make_peer(1, 8080);
    let peer2 = make_peer(2, 8080);
    let _peer1_id = peer1.node_id;
    let _peer2_id = peer2.node_id;

    table.stage_peer(peer1, now).unwrap();
    table.stage_peer(peer2, now).unwrap();

    let peer3 = make_peer(3, 8080);
    assert!(table.stage_peer(peer3, now).is_err());

    // INVARIANT-9: Tail Drop preserves existing pending peers (first-come-first-served)
    // Verify staging still has 2 peers (functionality tested, not field access)
    assert_eq!(table.pending_verification_count(), 2);
}

#[test]
fn test_staging_area_capacity_freed_after_verification_complete() {
    let local_id = make_node_id(0);
    let mut config = KademliaConfig::for_testing();
    config.max_pending_peers = 1;
    let mut table = RoutingTable::new(local_id, config);
    let now = Timestamp::new(1000);

    let peer1 = make_peer(1, 8080);
    let peer1_id = peer1.node_id;
    table.stage_peer(peer1, now).unwrap();

    // Staging at capacity
    let peer2 = make_peer(2, 8080);
    assert!(table.stage_peer(peer2.clone(), now).is_err());

    // Verification frees staging slot
    table.on_verification_result(&peer1_id, true, now).unwrap();

    // Slot now available
    assert!(table.stage_peer(peer2, now).is_ok());
}

#[test]
fn test_get_stats_reports_pending_verification_count() {
    let local_id = make_node_id(0);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let peer = make_peer(1, 8080);
    table.stage_peer(peer, now).unwrap();

    let stats = table.stats(now);
    assert_eq!(stats.pending_verification_count, 1);
    assert_eq!(stats.max_pending_peers, table.config().max_pending_peers);
}

// =============================================================================
// Test Group 8: Eviction-on-Failure (Eclipse Attack Defense)
// Reference: SPEC-01 Section 5.1 (V2.4 Eclipse Attack Defense Tests)
// =============================================================================

#[test]
fn test_table_poisoning_attack_is_blocked() {
    let local_id = make_node_id(0);
    let mut config = KademliaConfig::for_testing();
    config.k = 3;
    let mut table = RoutingTable::new(local_id, config);
    let now = Timestamp::new(1000);

    let make_bucket0_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
            Timestamp::new(1000),
        )
    };

    // Establish honest peer baseline
    let mut honest_peers = Vec::new();
    for i in 0..table.config().k {
        let peer = make_bucket0_peer(i as u8);
        honest_peers.push(peer.node_id);
        table.stage_peer(peer.clone(), now).unwrap();
        table
            .on_verification_result(&peer.node_id, true, now)
            .unwrap();
    }

    // Simulate attacker attempting table poisoning (20 malicious peers)
    // INVARIANT-10: All honest peers respond to challenges (alive)
    for i in 100..120 {
        let attacker_peer = make_bucket0_peer(i);
        table.stage_peer(attacker_peer.clone(), now).unwrap();

        match table.on_verification_result(&attacker_peer.node_id, true, now) {
            Ok(Some(challenged)) => {
                // Honest peer responds (alive) → attacker rejected
                table.on_challenge_response(&challenged, true, now).unwrap();
            }
            Err(PeerDiscoveryError::ChallengeInProgress) => {
                // Challenge already in progress per INVARIANT-10
            }
            _ => {}
        }
    }

    // SECURITY GUARANTEE: All honest peers survive attack
    let bucket_idx = calculate_bucket_index(&local_id, &honest_peers[0]);
    let bucket = table.get_bucket(bucket_idx).unwrap();

    for honest in &honest_peers {
        assert!(
            bucket.contains(honest),
            "Honest peer {:?} survives attack per INVARIANT-10",
            honest
        );
    }
    assert_eq!(
        bucket.len(),
        table.config().k,
        "Bucket maintains k peers after attack"
    );
}

// =============================================================================
// Test: Self-connection rejection (INVARIANT-5)
// =============================================================================

#[test]
fn test_bucket_rejects_self_node() {
    let local_id = make_node_id(42);
    let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
    let now = Timestamp::new(1000);

    let self_peer = PeerInfo::new(
        local_id,
        SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080),
        now,
    );

    // INVARIANT-5: Self-connection rejected
    let result = table.stage_peer(self_peer, now);

    assert!(
        matches!(result, Err(PeerDiscoveryError::SelfConnection)),
        "Self-connection rejected per INVARIANT-5"
    );
}

// =============================================================================
// Test: IP Diversity (INVARIANT-3)
// Reference: SPEC-01 Section 6.1 (Sybil Attack Resistance)
// =============================================================================

#[test]
fn test_rejects_third_peer_from_same_subnet() {
    let local_id = make_node_id(0);
    let mut config = KademliaConfig::for_testing();
    config.max_peers_per_subnet = 2;
    let mut table = RoutingTable::new(local_id, config);
    let now = Timestamp::new(1000);

    // All peers in same /24 subnet (192.168.1.0/24)
    let make_peer = |i: u8| {
        let mut bytes = [0u8; 32];
        bytes[0] = 0b1000_0000;
        bytes[1] = i;
        PeerInfo::new(
            NodeId::new(bytes),
            SocketAddr::new(IpAddr::v4(192, 168, 1, i), 8080),
            Timestamp::new(1000),
        )
    };

    // First two peers from same subnet accepted
    let peer1 = make_peer(1);
    let peer2 = make_peer(2);
    table.stage_peer(peer1.clone(), now).unwrap();
    table
        .on_verification_result(&peer1.node_id, true, now)
        .unwrap();

    table.stage_peer(peer2.clone(), now).unwrap();
    table
        .on_verification_result(&peer2.node_id, true, now)
        .unwrap();

    // INVARIANT-3: Third peer from same subnet rejected
    let peer3 = make_peer(3);
    table.stage_peer(peer3.clone(), now).unwrap();
    let result = table.on_verification_result(&peer3.node_id, true, now);

    assert!(
        matches!(result, Err(PeerDiscoveryError::SubnetLimitReached)),
        "Third peer from same /24 rejected per INVARIANT-3"
    );
}
