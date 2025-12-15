//! Peer scoring configuration.

use std::time::Duration;

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
            graylist_duration: Duration::from_secs(3600),
            blacklist_duration: Duration::from_secs(86400),
            decay_rate: 0.9,
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
