//! Peer score manager implementation.

use std::collections::HashMap;
use std::time::Duration;

use super::config::PeerScoreConfig;
use super::security::PeerScore;
use crate::domain::{NodeId, Timestamp};

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
        self.scores
            .get(node_id)
            .map(|s| s.is_graylistable(&self.config))
            .unwrap_or(false)
    }

    /// Check if peer should be blacklisted
    pub fn should_blacklist(&self, node_id: &NodeId) -> bool {
        self.scores
            .get(node_id)
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
        self.scores
            .iter()
            .filter(|(_, s)| s.is_graylistable(&self.config))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get peers that should be blacklisted
    pub fn get_blacklist_candidates(&self) -> Vec<NodeId> {
        self.scores
            .iter()
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
