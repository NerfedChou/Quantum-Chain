//! Peer scoring and penalty logic.
//!
//! SECURITY-CRITICAL: Contains spam protection scoring.
//! Isolate for security audits.

use super::config::PeerScoreConfig;
use crate::domain::Timestamp;

/// Score state for a single peer
///
/// # Security
/// This scoring system protects against:
/// - Spam: Invalid messages are heavily penalized
/// - DoS: Mesh delivery failures accumulate penalties
/// - Sybil: Time-in-mesh rewards long-standing peers
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
        let connection_minutes =
            now.as_secs().saturating_sub(self.connected_at.as_secs()) as f64 / 60.0;
        let time_bonus =
            (connection_minutes * config.time_in_mesh_weight).min(config.time_in_mesh_cap);

        // Apply decay (score regresses toward time_bonus baseline)
        self.score = self.score * config.decay_rate.powf(elapsed_minutes)
            + time_bonus * (1.0 - config.decay_rate.powf(elapsed_minutes));

        self.last_update = now;
    }
}
