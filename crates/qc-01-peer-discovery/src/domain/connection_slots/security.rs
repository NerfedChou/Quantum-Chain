//! Connection security and eviction logic.
//!
//! SECURITY-CRITICAL: Contains eviction protection and scoring logic.
//! Isolate for security audits.

use super::config::ConnectionSlotsConfig;
use super::types::ConnectionDirection;
use crate::domain::{NodeId, Timestamp};

/// Information about an active connection
///
/// # Security Note
/// This struct contains eviction protection logic. Peers with long
/// uptime or high scores are protected from eviction attacks.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Peer node ID
    pub node_id: NodeId,
    /// Whether this is an outbound (we dialed) or inbound (they dialed) connection
    pub direction: ConnectionDirection,
    /// When the connection was established
    pub connected_at: Timestamp,
    /// Current peer score (from PeerScoreManager)
    pub score: f64,
    /// Bytes received from this peer
    pub bytes_received: u64,
    /// Bytes sent to this peer
    pub bytes_sent: u64,
    /// Number of ping failures
    pub ping_failures: u32,
}

impl ConnectionInfo {
    /// Create a new connection info
    pub fn new(node_id: NodeId, direction: ConnectionDirection, now: Timestamp) -> Self {
        Self {
            node_id,
            direction,
            connected_at: now,
            score: 0.0,
            bytes_received: 0,
            bytes_sent: 0,
            ping_failures: 0,
        }
    }

    /// Calculate uptime in seconds
    pub fn uptime_secs(&self, now: Timestamp) -> u64 {
        now.as_secs().saturating_sub(self.connected_at.as_secs())
    }

    /// Check if this connection is protected from eviction
    ///
    /// # Security
    /// Protected peers cannot be evicted, preventing attackers from
    /// displacing long-standing honest connections.
    pub fn is_protected(&self, now: Timestamp, config: &ConnectionSlotsConfig) -> bool {
        self.uptime_secs(now) >= config.protection_threshold_secs
            || self.score >= config.protection_threshold_score
    }

    /// Calculate eviction score (lower = more likely to be evicted)
    ///
    /// # Security
    /// This heuristic determines which peer to evict. Higher scores
    /// mean better peers that should be retained.
    pub fn eviction_score(&self, now: Timestamp) -> f64 {
        let uptime_minutes = self.uptime_secs(now) as f64 / 60.0;
        let bandwidth_score = (self.bytes_received + self.bytes_sent) as f64 / 1_000_000.0;
        let ping_penalty = self.ping_failures as f64 * -2.0;

        self.score + (uptime_minutes * 0.1) + bandwidth_score + ping_penalty
    }
}
