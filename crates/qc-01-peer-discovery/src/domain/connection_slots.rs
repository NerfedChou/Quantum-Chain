//! # Connection Slots Management
//!
//! Implements deterministic slot reservation with Score-Based Eviction.
//!
//! ## Design (Bitcoin Core Inspired)
//!
//! - **Outbound Slots**: Sacred - only populated by our logic (Feeler → Tried → Dial)
//! - **Inbound Slots**: Populated by external peers dialing us
//!
//! ## Eviction Algorithm
//!
//! When inbound is full and a new peer connects:
//! 1. Scan existing inbound peers
//! 2. Identify "worst" peer by heuristic (shortest uptime, lowest score)
//! 3. If new peer is better, evict old and accept new
//! 4. Protected peers (long uptime, high score) are never evicted
//!
//! Reference: Bitcoin Core's `net.cpp` eviction logic

use std::collections::HashMap;

use crate::domain::{NodeId, Timestamp};

// =============================================================================
// CONFIGURATION
// =============================================================================

/// Connection slots configuration
#[derive(Debug, Clone)]
pub struct ConnectionSlotsConfig {
    /// Maximum outbound connections (sacred, never filled by inbound)
    pub max_outbound: usize,
    /// Maximum inbound connections
    pub max_inbound: usize,
    /// Minimum uptime (seconds) to be "protected" from eviction
    pub protection_threshold_secs: u64,
    /// Minimum score to be "protected" from eviction
    pub protection_threshold_score: f64,
    /// Maximum peers protected per eviction round
    pub max_protected_per_round: usize,
}

impl Default for ConnectionSlotsConfig {
    fn default() -> Self {
        Self {
            max_outbound: 10,
            max_inbound: 40,
            protection_threshold_secs: 3600, // 1 hour
            protection_threshold_score: 5.0,
            max_protected_per_round: 10,
        }
    }
}

impl ConnectionSlotsConfig {
    /// Testing config with smaller limits
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            max_outbound: 3,
            max_inbound: 5,
            protection_threshold_secs: 60,
            protection_threshold_score: 2.0,
            max_protected_per_round: 2,
        }
    }
}

// =============================================================================
// CONNECTION INFO
// =============================================================================

/// Information about an active connection
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
    pub fn is_protected(&self, now: Timestamp, config: &ConnectionSlotsConfig) -> bool {
        // Protected if long uptime OR high score
        self.uptime_secs(now) >= config.protection_threshold_secs
            || self.score >= config.protection_threshold_score
    }

    /// Calculate eviction score (lower = more likely to be evicted)
    /// This is the heuristic for "worst" peer
    pub fn eviction_score(&self, now: Timestamp) -> f64 {
        let uptime_minutes = self.uptime_secs(now) as f64 / 60.0;
        let bandwidth_score = (self.bytes_received + self.bytes_sent) as f64 / 1_000_000.0; // MB
        let ping_penalty = self.ping_failures as f64 * -2.0;

        // Higher score = better peer = less likely to evict
        self.score + (uptime_minutes * 0.1) + bandwidth_score + ping_penalty
    }
}

/// Direction of a connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionDirection {
    /// We initiated this connection (from Tried table)
    Outbound,
    /// Peer initiated this connection
    Inbound,
}

// =============================================================================
// CONNECTION SLOTS MANAGER
// =============================================================================

/// Manages connection slots with eviction logic
#[derive(Debug)]
pub struct ConnectionSlots {
    /// All active connections
    connections: HashMap<NodeId, ConnectionInfo>,
    /// Configuration
    config: ConnectionSlotsConfig,
}

impl ConnectionSlots {
    /// Create a new connection slots manager
    pub fn new(config: ConnectionSlotsConfig) -> Self {
        Self {
            connections: HashMap::new(),
            config,
        }
    }

    /// Get current outbound count
    pub fn outbound_count(&self) -> usize {
        self.connections
            .values()
            .filter(|c| c.direction == ConnectionDirection::Outbound)
            .count()
    }

    /// Get current inbound count
    pub fn inbound_count(&self) -> usize {
        self.connections
            .values()
            .filter(|c| c.direction == ConnectionDirection::Inbound)
            .count()
    }

    /// Check if we have outbound slots available
    pub fn has_outbound_slot(&self) -> bool {
        self.outbound_count() < self.config.max_outbound
    }

    /// Check if we have inbound slots available (without eviction)
    pub fn has_inbound_slot(&self) -> bool {
        self.inbound_count() < self.config.max_inbound
    }

    /// Reserve an outbound slot for dialing
    ///
    /// Returns true if slot was reserved, false if no slots available.
    /// Outbound slots are SACRED - never displaced by inbound.
    pub fn reserve_outbound(&mut self, node_id: NodeId, now: Timestamp) -> bool {
        // Check if already connected
        if self.connections.contains_key(&node_id) {
            return false;
        }

        // Check if slots available
        if !self.has_outbound_slot() {
            return false;
        }

        let conn = ConnectionInfo::new(node_id, ConnectionDirection::Outbound, now);
        self.connections.insert(node_id, conn);
        true
    }

    /// Try to accept an inbound connection
    ///
    /// Returns:
    /// - `AcceptResult::Accepted` - Connection accepted
    /// - `AcceptResult::Rejected` - No slots and eviction failed
    /// - `AcceptResult::Evicted(NodeId)` - Accepted after evicting another peer
    pub fn try_accept_inbound(
        &mut self,
        node_id: NodeId,
        score: f64,
        now: Timestamp,
    ) -> AcceptResult {
        // Check if already connected
        if self.connections.contains_key(&node_id) {
            return AcceptResult::Rejected;
        }

        // If slots available, accept directly
        if self.has_inbound_slot() {
            let mut conn = ConnectionInfo::new(node_id, ConnectionDirection::Inbound, now);
            conn.score = score;
            self.connections.insert(node_id, conn);
            return AcceptResult::Accepted;
        }

        // Slots full - try eviction
        if let Some(victim) = self.find_eviction_candidate(score, now) {
            // Evict the worst peer
            self.connections.remove(&victim);

            // Accept the new peer
            let mut conn = ConnectionInfo::new(node_id, ConnectionDirection::Inbound, now);
            conn.score = score;
            self.connections.insert(node_id, conn);

            return AcceptResult::Evicted(victim);
        }

        // New peer isn't better than any existing peer
        AcceptResult::Rejected
    }

    /// Find a candidate for eviction
    ///
    /// Returns the NodeId of the "worst" unprotected inbound peer
    /// if the new peer (with given score) would be better.
    fn find_eviction_candidate(&self, new_peer_score: f64, now: Timestamp) -> Option<NodeId> {
        // Get all inbound peers that are NOT protected
        let mut candidates: Vec<_> = self
            .connections
            .values()
            .filter(|c| c.direction == ConnectionDirection::Inbound)
            .filter(|c| !c.is_protected(now, &self.config))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        // Sort by eviction score (ascending = worst first)
        candidates.sort_by(|a, b| {
            a.eviction_score(now)
                .partial_cmp(&b.eviction_score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Get the worst peer
        let worst = candidates.first()?;

        // Only evict if new peer is better
        // New peer's eviction score estimate (just score, no uptime/bandwidth yet)
        if new_peer_score > worst.eviction_score(now) {
            Some(worst.node_id)
        } else {
            None
        }
    }

    /// Disconnect a peer (inbound or outbound)
    pub fn disconnect(&mut self, node_id: &NodeId) -> Option<ConnectionInfo> {
        self.connections.remove(node_id)
    }

    /// Update a peer's score
    pub fn update_score(&mut self, node_id: &NodeId, score: f64) {
        if let Some(conn) = self.connections.get_mut(node_id) {
            conn.score = score;
        }
    }

    /// Record bytes received from a peer
    pub fn record_bytes_received(&mut self, node_id: &NodeId, bytes: u64) {
        if let Some(conn) = self.connections.get_mut(node_id) {
            conn.bytes_received = conn.bytes_received.saturating_add(bytes);
        }
    }

    /// Record bytes sent to a peer
    pub fn record_bytes_sent(&mut self, node_id: &NodeId, bytes: u64) {
        if let Some(conn) = self.connections.get_mut(node_id) {
            conn.bytes_sent = conn.bytes_sent.saturating_add(bytes);
        }
    }

    /// Record a ping failure
    pub fn record_ping_failure(&mut self, node_id: &NodeId) {
        if let Some(conn) = self.connections.get_mut(node_id) {
            conn.ping_failures += 1;
        }
    }

    /// Get connection info for a peer
    pub fn get(&self, node_id: &NodeId) -> Option<&ConnectionInfo> {
        self.connections.get(node_id)
    }

    /// Check if a peer is connected
    pub fn is_connected(&self, node_id: &NodeId) -> bool {
        self.connections.contains_key(node_id)
    }

    /// Get all connected peer IDs
    pub fn connected_peers(&self) -> Vec<NodeId> {
        self.connections.keys().copied().collect()
    }

    /// Get statistics
    pub fn stats(&self) -> ConnectionStats {
        ConnectionStats {
            outbound_count: self.outbound_count(),
            inbound_count: self.inbound_count(),
            max_outbound: self.config.max_outbound,
            max_inbound: self.config.max_inbound,
        }
    }
}

/// Result of trying to accept an inbound connection
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptResult {
    /// Connection accepted (slot was available)
    Accepted,
    /// Connection rejected (no slots, eviction failed)
    Rejected,
    /// Connection accepted after evicting another peer
    Evicted(NodeId),
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub outbound_count: usize,
    pub inbound_count: usize,
    pub max_outbound: usize,
    pub max_inbound: usize,
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node_id(byte: u8) -> NodeId {
        let mut id = [0u8; 32];
        id[0] = byte;
        NodeId::new(id)
    }

    // =========================================================================
    // TEST GROUP 1: Basic Slot Reservation
    // =========================================================================

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

    // =========================================================================
    // TEST GROUP 2: Inbound Cannot Displace Outbound
    // =========================================================================

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
        let result = slots.try_accept_inbound(new, 100.0, now); // High score

        // Should not have affected outbound count
        assert_eq!(slots.outbound_count(), config.max_outbound);
    }

    // =========================================================================
    // TEST GROUP 3: Eviction Logic
    // =========================================================================

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

    // =========================================================================
    // TEST GROUP 4: Disconnect and Statistics
    // =========================================================================

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

    // =========================================================================
    // TEST GROUP 5: Score and Bandwidth Tracking
    // =========================================================================

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
        let result = slots.try_accept_inbound(new, 3.0, now);

        // Should still be connected (was protected by score update)
        assert!(slots.is_connected(&victim));
    }
}
