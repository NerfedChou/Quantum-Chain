//! Banned peers tracking.
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.2

use super::security::BannedEntry;
use crate::domain::{BanReason, NodeId, Timestamp};
use std::collections::HashMap;

/// Tracks banned peers with expiration times
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug, Clone, Default)]
pub struct BannedPeers {
    entries: HashMap<NodeId, BannedEntry>,
}

impl BannedPeers {
    /// Create a new empty banned peers tracker.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a ban entry
    pub fn ban(&mut self, node_id: NodeId, until: Timestamp, reason: BanReason) {
        self.entries.insert(
            node_id,
            BannedEntry {
                node_id,
                banned_until: until,
                reason,
            },
        );
    }

    /// Check if a peer is currently banned.
    pub fn is_banned(&self, node_id: &NodeId, now: Timestamp) -> bool {
        self.entries
            .get(node_id)
            .is_some_and(|entry| entry.banned_until > now)
    }

    /// Remove expired bans
    pub fn gc_expired(&mut self, now: Timestamp) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.banned_until > now);
        before - self.entries.len()
    }

    /// Get count of active bans
    pub fn count(&self, now: Timestamp) -> usize {
        self.entries
            .values()
            .filter(|e| e.banned_until > now)
            .count()
    }
}
