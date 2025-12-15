//! K-Bucket implementation for Kademlia routing.
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.2

use super::security::PendingInsertion;
use crate::domain::{NodeId, PeerInfo, Timestamp};

/// A k-bucket storing up to k peers at a specific distance range
///
/// # Security (Eclipse Attack Defense - V2.4 Eviction-on-Failure)
/// When the bucket is full and a new verified peer wants to join, we do NOT
/// immediately evict the oldest peer. Instead, we CHALLENGE the oldest peer
/// with a PING. Only if the oldest peer fails to respond (is dead) do we evict.
/// This prevents "Table Poisoning" attacks where an attacker sequentially
/// connects with 20 new nodes to flush honest, stable peers.
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug, Clone)]
pub struct KBucket {
    /// Peers in this bucket (max size = K, default 20)
    pub(crate) peers: Vec<PeerInfo>,
    /// Last time this bucket was updated
    pub(crate) last_updated: Timestamp,
    /// Peer waiting to join this bucket, pending eviction challenge result.
    pub(crate) pending_insertion: Option<PendingInsertion>,
}

impl KBucket {
    /// Create a new empty k-bucket
    pub fn new() -> Self {
        Self {
            peers: Vec::new(),
            last_updated: Timestamp::new(0),
            pending_insertion: None,
        }
    }

    /// Get the number of peers in this bucket
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// Check if the bucket is empty
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Check if the bucket is full
    pub fn is_full(&self, k: usize) -> bool {
        self.peers.len() >= k
    }

    /// Get the oldest peer (least recently seen)
    pub fn oldest_peer(&self) -> Option<&PeerInfo> {
        self.peers.first()
    }

    /// Get all peers in this bucket
    pub fn peers(&self) -> &[PeerInfo] {
        &self.peers
    }

    /// Check if a challenge is already in progress
    pub fn has_pending_challenge(&self) -> bool {
        self.pending_insertion.is_some()
    }

    /// Add a peer to the bucket (assumes not full)
    ///
    /// New peers are added to the end (most recently seen position)
    pub(crate) fn add_peer(&mut self, peer: PeerInfo, now: Timestamp) {
        self.peers.push(peer);
        self.last_updated = now;
    }

    /// Remove a peer by NodeId using optimized position-map pattern.
    pub(crate) fn remove_peer(&mut self, node_id: &NodeId) -> Option<PeerInfo> {
        self.peers
            .iter()
            .position(|p| &p.node_id == node_id)
            .map(|pos| self.peers.remove(pos))
    }

    /// Move a peer to the front (most recently seen)
    pub(crate) fn move_to_front(&mut self, node_id: &NodeId, now: Timestamp) -> bool {
        if let Some(pos) = self.peers.iter().position(|p| &p.node_id == node_id) {
            let mut peer = self.peers.remove(pos);
            peer.last_seen = now;
            self.peers.push(peer);
            self.last_updated = now;
            true
        } else {
            false
        }
    }

    /// Update a peer's last_seen timestamp
    pub(crate) fn touch_peer(&mut self, node_id: &NodeId, now: Timestamp) -> bool {
        if let Some(peer) = self.peers.iter_mut().find(|p| &p.node_id == node_id) {
            peer.last_seen = now;
            self.last_updated = now;
            true
        } else {
            false
        }
    }

    /// Check if bucket contains a peer
    pub(crate) fn contains(&self, node_id: &NodeId) -> bool {
        self.peers.iter().any(|p| &p.node_id == node_id)
    }
}

impl Default for KBucket {
    fn default() -> Self {
        Self::new()
    }
}
