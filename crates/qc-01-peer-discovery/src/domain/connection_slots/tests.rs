//! Tests for Connection Slots Management
//!
//! Reference: Bitcoin Core's `net.cpp` eviction logic

use super::*;
use crate::domain::{NodeId, Timestamp};

fn make_node_id(byte: u8) -> NodeId {
    let mut id = [0u8; 32];
    id[0] = byte;
    NodeId::new(id)
}

// =============================================================================
// TEST GROUP 1: Basic Slot Reservation
// =============================================================================

#[test]
fn test_outbound_reservation() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Should be able to reserve up to max_outbound
    for i in 0..config.max_outbound {
        let node = make_node_id(i as u8);
        assert!(slots.reserve_outbound(node, now));
    }

    // Next reservation should fail
    let extra = make_node_id(100);
    assert!(!slots.reserve_outbound(extra, now));

    assert_eq!(slots.outbound_count(), config.max_outbound);
}

#[test]
fn test_inbound_acceptance() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Should accept up to max_inbound
    for i in 0..config.max_inbound {
        let node = make_node_id(i as u8);
        let result = slots.try_accept_inbound(node, 0.0, now);
        assert_eq!(result, AcceptResult::Accepted);
    }

    assert_eq!(slots.inbound_count(), config.max_inbound);
}

// =============================================================================
// TEST GROUP 2: Inbound Cannot Displace Outbound
// =============================================================================

#[test]
fn test_inbound_never_displaces_outbound() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Fill all outbound slots
    for i in 0..config.max_outbound {
        let node = make_node_id(i as u8);
        slots.reserve_outbound(node, now);
    }

    // Fill all inbound slots
    for i in 0..config.max_inbound {
        let node = make_node_id((100 + i) as u8);
        slots.try_accept_inbound(node, 0.0, now);
    }

    // New inbound should be rejected (not evict outbound)
    let new = make_node_id(200);
    let _result = slots.try_accept_inbound(new, 100.0, now); // High score

    // Should not have affected outbound count
    assert_eq!(slots.outbound_count(), config.max_outbound);
}

// =============================================================================
// TEST GROUP 3: Eviction Logic
// =============================================================================

#[test]
fn test_eviction_of_worst_peer() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Fill inbound with low-score peers
    for i in 0..config.max_inbound {
        let node = make_node_id(i as u8);
        slots.try_accept_inbound(node, -1.0, now); // Negative score = bad
    }

    // New peer with high score should evict worst
    let new = make_node_id(100);
    let result = slots.try_accept_inbound(new, 10.0, now);

    assert!(matches!(result, AcceptResult::Evicted(_)));
    assert!(slots.is_connected(&new));
    assert_eq!(slots.inbound_count(), config.max_inbound);
}

#[test]
fn test_protected_peer_not_evicted() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Add peers with high scores (protected)
    for i in 0..config.max_inbound {
        let node = make_node_id(i as u8);
        slots.try_accept_inbound(node, 10.0, now); // High score = protected
    }

    // New peer should be rejected (all existing are protected)
    let new = make_node_id(100);
    let result = slots.try_accept_inbound(new, 5.0, now);

    assert_eq!(result, AcceptResult::Rejected);
    assert!(!slots.is_connected(&new));
}

// =============================================================================
// TEST GROUP 4: Disconnect and Statistics
// =============================================================================

#[test]
fn test_disconnect_frees_slot() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config);
    let now = Timestamp::new(1000);

    let node = make_node_id(1);
    slots.reserve_outbound(node, now);
    assert_eq!(slots.outbound_count(), 1);

    slots.disconnect(&node);
    assert_eq!(slots.outbound_count(), 0);
    assert!(slots.has_outbound_slot());
}

#[test]
fn test_stats() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    slots.reserve_outbound(make_node_id(1), now);
    slots.reserve_outbound(make_node_id(2), now);
    slots.try_accept_inbound(make_node_id(10), 0.0, now);

    let stats = slots.stats();
    assert_eq!(stats.outbound_count, 2);
    assert_eq!(stats.inbound_count, 1);
    assert_eq!(stats.max_outbound, config.max_outbound);
    assert_eq!(stats.max_inbound, config.max_inbound);
}

// =============================================================================
// TEST GROUP 5: Score and Bandwidth Tracking
// =============================================================================

#[test]
fn test_score_update_affects_eviction() {
    let config = ConnectionSlotsConfig::for_testing();
    let mut slots = ConnectionSlots::new(config.clone());
    let now = Timestamp::new(1000);

    // Fill slots with medium-score peers
    let victim = make_node_id(1);
    slots.try_accept_inbound(victim, 1.0, now);

    for i in 2..=config.max_inbound {
        slots.try_accept_inbound(make_node_id(i as u8), 5.0, now);
    }

    // Update victim's score to be protected
    slots.update_score(&victim, 10.0);

    // New peer should NOT evict the now-protected victim
    let new = make_node_id(100);
    let _result = slots.try_accept_inbound(new, 3.0, now);

    // Should still be connected (was protected by score update)
    assert!(slots.is_connected(&victim));
}
