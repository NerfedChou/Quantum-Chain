//! Tests for Peer Scoring System
//!
//! Reference: Libp2p GossipSub v1.1 Peer Scoring

use super::*;
use crate::domain::{NodeId, Timestamp};

fn make_node_id(byte: u8) -> NodeId {
    let mut id = [0u8; 32];
    id[0] = byte;
    NodeId::new(id)
}

// =============================================================================
// TEST HELPERS
// =============================================================================

fn setup_manager_with_node() -> (PeerScoreManager, PeerScoreConfig, NodeId, Timestamp) {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config.clone());
    let now = Timestamp::new(1000);
    let node = make_node_id(1);
    manager.on_peer_connected(node, now);
    (manager, config, node, now)
}

// =============================================================================
// TEST GROUP 1: Basic Scoring
// =============================================================================

#[test]
fn test_new_peer_starts_at_zero() {
    let (manager, _, node, _) = setup_manager_with_node();

    let score = manager.get_score(&node);
    assert!(score.is_some());
    assert_eq!(score.unwrap(), 0.0);
}

#[test]
fn test_first_block_delivery_increases_score() {
    let (mut manager, config, node, _) = setup_manager_with_node();

    manager.on_first_block_delivery(&node);

    let score = manager.get_score(&node).unwrap();
    assert_eq!(score, config.first_block_delivery_weight);
}

#[test]
fn test_invalid_block_decreases_score() {
    let (mut manager, config, node, _) = setup_manager_with_node();

    manager.on_invalid_block(&node);

    let score = manager.get_score(&node).unwrap();
    assert_eq!(score, config.invalid_block_penalty);
}

// =============================================================================
// TEST GROUP 2: Graylist/Blacklist Thresholds
// =============================================================================

#[test]
fn test_graylist_when_score_below_zero() {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config);
    let now = Timestamp::new(1000);

    let node = make_node_id(1);
    manager.on_peer_connected(node, now);

    // Score is 0, should NOT be graylisted
    assert!(!manager.should_graylist(&node));

    // One invalid block drops below 0
    manager.on_invalid_block(&node);
    assert!(manager.should_graylist(&node));
}

#[test]
fn test_blacklist_when_score_below_threshold() {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config.clone());
    let now = Timestamp::new(1000);

    let node = make_node_id(1);
    manager.on_peer_connected(node, now);

    // Multiple invalid blocks to drop below blacklist threshold (-50)
    // Each invalid block is -10, need 6 to get to -60
    manager.on_invalid_block(&node); // -10
    manager.on_invalid_block(&node); // -20
    manager.on_invalid_block(&node); // -30
    manager.on_invalid_block(&node); // -40
    manager.on_invalid_block(&node); // -50
    manager.on_invalid_block(&node); // -60 (below -50 threshold)

    assert!(manager.should_blacklist(&node));
}

// =============================================================================
// TEST GROUP 3: Positive Behavior Rewards
// =============================================================================

#[test]
fn test_multiple_good_actions_accumulate() {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config.clone());
    let now = Timestamp::new(1000);

    let node = make_node_id(1);
    manager.on_peer_connected(node, now);

    // Multiple good actions
    manager.on_first_block_delivery(&node); // +5
    manager.on_first_block_delivery(&node); // +5
    manager.on_first_tx_delivery(&node); // +0.5

    let score = manager.get_score(&node).unwrap();
    assert_eq!(score, 10.5);
}

// =============================================================================
// TEST GROUP 4: Candidate Lists
// =============================================================================

#[test]
fn test_get_graylist_candidates() {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config);
    let now = Timestamp::new(1000);

    let good_node = make_node_id(1);
    let bad_node = make_node_id(2);

    manager.on_peer_connected(good_node, now);
    manager.on_peer_connected(bad_node, now);

    manager.on_first_block_delivery(&good_node); // +5
    manager.on_invalid_block(&bad_node); // -10

    let candidates = manager.get_graylist_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0], bad_node);
}

// =============================================================================
// TEST GROUP 5: Disconnection
// =============================================================================

#[test]
fn test_disconnected_peer_removed() {
    let config = PeerScoreConfig::for_testing();
    let mut manager = PeerScoreManager::new(config);
    let now = Timestamp::new(1000);

    let node = make_node_id(1);
    manager.on_peer_connected(node, now);
    manager.on_peer_disconnected(&node);

    assert!(manager.get_score(&node).is_none());
}
