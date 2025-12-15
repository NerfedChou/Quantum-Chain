//! Connection slots manager implementation.

use std::collections::HashMap;

use super::config::ConnectionSlotsConfig;
use super::security::ConnectionInfo;
use super::types::{AcceptResult, ConnectionDirection, ConnectionStats};
use crate::domain::{NodeId, Timestamp};

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
        if self.connections.contains_key(&node_id) {
            return false;
        }

        if !self.has_outbound_slot() {
            return false;
        }

        let conn = ConnectionInfo::new(node_id, ConnectionDirection::Outbound, now);
        self.connections.insert(node_id, conn);
        true
    }

    /// Try to accept an inbound connection
    pub fn try_accept_inbound(
        &mut self,
        node_id: NodeId,
        score: f64,
        now: Timestamp,
    ) -> AcceptResult {
        if self.connections.contains_key(&node_id) {
            return AcceptResult::Rejected;
        }

        if self.has_inbound_slot() {
            let mut conn = ConnectionInfo::new(node_id, ConnectionDirection::Inbound, now);
            conn.score = score;
            self.connections.insert(node_id, conn);
            return AcceptResult::Accepted;
        }

        if let Some(victim) = self.find_eviction_candidate(score, now) {
            self.connections.remove(&victim);

            let mut conn = ConnectionInfo::new(node_id, ConnectionDirection::Inbound, now);
            conn.score = score;
            self.connections.insert(node_id, conn);

            return AcceptResult::Evicted(victim);
        }

        AcceptResult::Rejected
    }

    /// Find a candidate for eviction
    fn find_eviction_candidate(&self, new_peer_score: f64, now: Timestamp) -> Option<NodeId> {
        let mut candidates: Vec<_> = self
            .connections
            .values()
            .filter(|c| c.direction == ConnectionDirection::Inbound)
            .filter(|c| !c.is_protected(now, &self.config))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        candidates.sort_by(|a, b| {
            a.eviction_score(now)
                .partial_cmp(&b.eviction_score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let worst = candidates.first()?;

        if new_peer_score > worst.eviction_score(now) {
            Some(worst.node_id)
        } else {
            None
        }
    }

    /// Disconnect a peer
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
