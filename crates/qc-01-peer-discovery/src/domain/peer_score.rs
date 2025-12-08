//! # Peer Scoring System (Gossip Scoring)
//!
//! Implements Libp2p-style peer scoring for spam protection and quality filtering.
//!
//! ## Scoring Parameters
//!
//! - **P1 (Time in Mesh)**: Reward stable, long-connected peers
//! - **P2 (First Message Delivery)**: Reward peers who deliver new blocks/txs first
//! - **P3 (Invalid Messages)**: Heavily penalize invalid block/signature senders
//! - **P4 (Mesh Delivery Failure)**: Penalize unreliable message relay
//!
//! ## Thresholds
//!
//! - Score < 0: Graylist (disconnect, ignore for 1 hour)
//! - Score < -100: Blacklist (ban for 24 hours)
//!
//! Reference: Libp2p GossipSub v1.1 Peer Scoring

use std::collections::HashMap;
use std::time::Duration;

use crate::domain::{NodeId, Timestamp};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Peer scoring configuration
#[derive(Debug, Clone)]
pub struct PeerScoreConfig {
    /// P1: Points per minute connected (max 10)
    pub time_in_mesh_weight: f64,
    /// P1: Maximum bonus from time in mesh
    pub time_in_mesh_cap: f64,
    
    /// P2: Points per first valid block delivery
    pub first_block_delivery_weight: f64,
    /// P2: Points per first valid tx delivery
    pub first_tx_delivery_weight: f64,
    
    /// P3: Points per invalid block (negative)
    pub invalid_block_penalty: f64,
    /// P3: Points per invalid signature (negative)
    pub invalid_signature_penalty: f64,
    
    /// P4: Points per mesh delivery failure (negative)
    pub mesh_failure_penalty: f64,
    
    /// Score below which peer is graylisted
    pub graylist_threshold: f64,
    /// Score below which peer is blacklisted
    pub blacklist_threshold: f64,
    
    /// How long a graylisted peer is ignored
    pub graylist_duration: Duration,
    /// How long a blacklisted peer is banned
    pub blacklist_duration: Duration,
    
    /// Score decay per minute (regression to mean)
    pub decay_rate: f64,
}

impl Default for PeerScoreConfig {
    fn default() -> Self {
        Self {
            time_in_mesh_weight: 0.1,
            time_in_mesh_cap: 10.0,
            first_block_delivery_weight: 5.0,
            first_tx_delivery_weight: 0.5,
            invalid_block_penalty: -50.0,
            invalid_signature_penalty: -100.0,
            mesh_failure_penalty: -1.0,
            graylist_threshold: 0.0,
            blacklist_threshold: -100.0,
            graylist_duration: Duration::from_secs(3600),      // 1 hour
            blacklist_duration: Duration::from_secs(86400),    // 24 hours
            decay_rate: 0.9, // Score decays towards 0
        }
    }
}

impl PeerScoreConfig {
    /// Testing config with smaller penalties
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            time_in_mesh_weight: 1.0,
            time_in_mesh_cap: 10.0,
            first_block_delivery_weight: 5.0,
            first_tx_delivery_weight: 0.5,
            invalid_block_penalty: -10.0,
            invalid_signature_penalty: -20.0,
            mesh_failure_penalty: -1.0,
            graylist_threshold: 0.0,
            blacklist_threshold: -50.0,
            graylist_duration: Duration::from_secs(60),
            blacklist_duration: Duration::from_secs(300),
            decay_rate: 0.9,
        }
    }
}

// =============================================================================
// PEER SCORE
// =============================================================================

/// Score state for a single peer
#[derive(Debug, Clone)]
pub struct PeerScore {
    /// Current aggregated score
    score: f64,
    /// When peer first connected
    connected_at: Timestamp,
    /// Number of first valid block deliveries
    first_block_deliveries: u32,
    /// Number of first valid tx deliveries
    first_tx_deliveries: u32,
    /// Number of invalid blocks sent
    invalid_blocks: u32,
    /// Number of invalid signatures
    invalid_signatures: u32,
    /// Number of mesh delivery failures
    mesh_failures: u32,
    /// Last score update
    last_update: Timestamp,
}

impl PeerScore {
    /// Create a new peer score (starts at 0)
    pub fn new(connected_at: Timestamp) -> Self {
        Self {
            score: 0.0,
            connected_at,
            first_block_deliveries: 0,
            first_tx_deliveries: 0,
            invalid_blocks: 0,
            invalid_signatures: 0,
            mesh_failures: 0,
            last_update: connected_at,
        }
    }

    /// Get current score
    pub fn score(&self) -> f64 {
        self.score
    }

    /// Check if peer should be graylisted
    pub fn is_graylistable(&self, config: &PeerScoreConfig) -> bool {
        self.score < config.graylist_threshold
    }

    /// Check if peer should be blacklisted
    pub fn is_blacklistable(&self, config: &PeerScoreConfig) -> bool {
        self.score < config.blacklist_threshold
    }

    /// Record first valid block delivery (+5.0)
    pub fn on_first_block_delivery(&mut self, config: &PeerScoreConfig) {
        self.first_block_deliveries += 1;
        self.score += config.first_block_delivery_weight;
    }

    /// Record first valid transaction delivery (+0.5)
    pub fn on_first_tx_delivery(&mut self, config: &PeerScoreConfig) {
        self.first_tx_deliveries += 1;
        self.score += config.first_tx_delivery_weight;
    }

    /// Record invalid block received (-50.0)
    pub fn on_invalid_block(&mut self, config: &PeerScoreConfig) {
        self.invalid_blocks += 1;
        self.score += config.invalid_block_penalty;
    }

    /// Record invalid signature (-100.0)
    pub fn on_invalid_signature(&mut self, config: &PeerScoreConfig) {
        self.invalid_signatures += 1;
        self.score += config.invalid_signature_penalty;
    }

    /// Record mesh delivery failure (-1.0)
    pub fn on_mesh_failure(&mut self, config: &PeerScoreConfig) {
        self.mesh_failures += 1;
        self.score += config.mesh_failure_penalty;
    }

    /// Update time-in-mesh bonus and apply decay
    pub fn update(&mut self, now: Timestamp, config: &PeerScoreConfig) {
        let elapsed_secs = now.as_secs().saturating_sub(self.last_update.as_secs());
        if elapsed_secs == 0 {
            return;
        }

        let elapsed_minutes = elapsed_secs as f64 / 60.0;

        // P1: Time in mesh bonus
        let connection_minutes = now.as_secs().saturating_sub(self.connected_at.as_secs()) as f64 / 60.0;
        let time_bonus = (connection_minutes * config.time_in_mesh_weight).min(config.time_in_mesh_cap);

        // Apply decay (score regresses toward time_bonus baseline)
        self.score = self.score * config.decay_rate.powf(elapsed_minutes) + time_bonus * (1.0 - config.decay_rate.powf(elapsed_minutes));

        self.last_update = now;
    }
}

// =============================================================================
// PEER SCORE MANAGER
// =============================================================================

/// Manages scores for all peers
#[derive(Debug)]
pub struct PeerScoreManager {
    /// Scores per peer
    scores: HashMap<NodeId, PeerScore>,
    /// Configuration
    config: PeerScoreConfig,
}

impl PeerScoreManager {
    /// Create a new score manager
    pub fn new(config: PeerScoreConfig) -> Self {
        Self {
            scores: HashMap::new(),
            config,
        }
    }

    /// Register a new peer
    pub fn on_peer_connected(&mut self, node_id: NodeId, now: Timestamp) {
        self.scores.insert(node_id, PeerScore::new(now));
    }

    /// Remove a peer
    pub fn on_peer_disconnected(&mut self, node_id: &NodeId) {
        self.scores.remove(node_id);
    }

    /// Get a peer's current score
    pub fn get_score(&self, node_id: &NodeId) -> Option<f64> {
        self.scores.get(node_id).map(|s| s.score())
    }

    /// Check if peer should be graylisted
    pub fn should_graylist(&self, node_id: &NodeId) -> bool {
        self.scores.get(node_id)
            .map(|s| s.is_graylistable(&self.config))
            .unwrap_or(false)
    }

    /// Check if peer should be blacklisted
    pub fn should_blacklist(&self, node_id: &NodeId) -> bool {
        self.scores.get(node_id)
            .map(|s| s.is_blacklistable(&self.config))
            .unwrap_or(false)
    }

    /// Record first valid block delivery
    pub fn on_first_block_delivery(&mut self, node_id: &NodeId) {
        if let Some(score) = self.scores.get_mut(node_id) {
            score.on_first_block_delivery(&self.config);
        }
    }

    /// Record first valid tx delivery
    pub fn on_first_tx_delivery(&mut self, node_id: &NodeId) {
        if let Some(score) = self.scores.get_mut(node_id) {
            score.on_first_tx_delivery(&self.config);
        }
    }

    /// Record invalid block
    pub fn on_invalid_block(&mut self, node_id: &NodeId) {
        if let Some(score) = self.scores.get_mut(node_id) {
            score.on_invalid_block(&self.config);
        }
    }

    /// Record invalid signature
    pub fn on_invalid_signature(&mut self, node_id: &NodeId) {
        if let Some(score) = self.scores.get_mut(node_id) {
            score.on_invalid_signature(&self.config);
        }
    }

    /// Record mesh failure
    pub fn on_mesh_failure(&mut self, node_id: &NodeId) {
        if let Some(score) = self.scores.get_mut(node_id) {
            score.on_mesh_failure(&self.config);
        }
    }

    /// Update all peer scores (call periodically)
    pub fn update_all(&mut self, now: Timestamp) {
        for score in self.scores.values_mut() {
            score.update(now, &self.config);
        }
    }

    /// Get peers that should be graylisted
    pub fn get_graylist_candidates(&self) -> Vec<NodeId> {
        self.scores.iter()
            .filter(|(_, s)| s.is_graylistable(&self.config))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get peers that should be blacklisted
    pub fn get_blacklist_candidates(&self) -> Vec<NodeId> {
        self.scores.iter()
            .filter(|(_, s)| s.is_blacklistable(&self.config))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get graylist duration from config
    pub fn graylist_duration(&self) -> Duration {
        self.config.graylist_duration
    }

    /// Get blacklist duration from config
    pub fn blacklist_duration(&self) -> Duration {
        self.config.blacklist_duration
    }
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NodeId;

    fn make_node_id(byte: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = byte;
        NodeId::new(id)
    }

    // =========================================================================
    // TEST GROUP 1: Basic Scoring
    // =========================================================================

    #[test]
    fn test_new_peer_starts_at_zero() {
        let config = PeerScoreConfig::for_testing();
        let mut manager = PeerScoreManager::new(config);
        let now = Timestamp::new(1000);

        let node = make_node_id(1);
        manager.on_peer_connected(node, now);

        let score = manager.get_score(&node);
        assert!(score.is_some());
        assert_eq!(score.unwrap(), 0.0);
    }

    #[test]
    fn test_first_block_delivery_increases_score() {
        let config = PeerScoreConfig::for_testing();
        let mut manager = PeerScoreManager::new(config.clone());
        let now = Timestamp::new(1000);

        let node = make_node_id(1);
        manager.on_peer_connected(node, now);
        manager.on_first_block_delivery(&node);

        let score = manager.get_score(&node).unwrap();
        assert_eq!(score, config.first_block_delivery_weight);
    }

    #[test]
    fn test_invalid_block_decreases_score() {
        let config = PeerScoreConfig::for_testing();
        let mut manager = PeerScoreManager::new(config.clone());
        let now = Timestamp::new(1000);

        let node = make_node_id(1);
        manager.on_peer_connected(node, now);
        manager.on_invalid_block(&node);

        let score = manager.get_score(&node).unwrap();
        assert_eq!(score, config.invalid_block_penalty);
    }

    // =========================================================================
    // TEST GROUP 2: Graylist/Blacklist Thresholds
    // =========================================================================

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

    // =========================================================================
    // TEST GROUP 3: Positive Behavior Rewards
    // =========================================================================

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
        manager.on_first_tx_delivery(&node);   // +0.5

        let score = manager.get_score(&node).unwrap();
        assert_eq!(score, 10.5);
    }

    // =========================================================================
    // TEST GROUP 4: Candidate Lists
    // =========================================================================

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
        manager.on_invalid_block(&bad_node);        // -10

        let candidates = manager.get_graylist_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], bad_node);
    }

    // =========================================================================
    // TEST GROUP 5: Disconnection
    // =========================================================================

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
}
