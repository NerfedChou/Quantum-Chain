//! Routing table security types.
//!
//! SECURITY-CRITICAL: Contains types for staging, verification, banning.
//! Isolate for security audits.

use crate::domain::{BanReason, NodeId, PeerInfo, Timestamp};

/// A peer waiting to be inserted into a full bucket, pending challenge result
///
/// # Security (V2.4)
/// This enables the "Eviction-on-Failure" policy.
/// The new peer only gets inserted if the challenged (oldest) peer is dead.
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug, Clone)]
pub struct PendingInsertion {
    /// The new peer waiting to be inserted
    pub candidate: PeerInfo,
    /// The existing peer being challenged (oldest/least-recently-seen)
    pub challenged_peer: NodeId,
    /// When the challenge was sent
    pub challenge_sent_at: Timestamp,
    /// Deadline for challenge response
    pub challenge_deadline: Timestamp,
}

/// A peer awaiting identity verification from Subsystem 10
///
/// # Security (DDoS Edge Defense)
/// Peers must pass verification before entering the routing table.
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug, Clone)]
pub struct PendingPeer {
    /// The peer's information
    pub peer_info: PeerInfo,
    /// When we received this peer
    pub received_at: Timestamp,
    /// Timeout for verification (after which peer is dropped)
    pub verification_deadline: Timestamp,
}

/// Individual ban entry.
///
/// # Security
/// Banned peers are tracked to prevent re-connection attempts.
#[derive(Debug, Clone)]
pub struct BannedEntry {
    /// The banned node's ID.
    pub node_id: NodeId,
    /// When the ban expires.
    pub banned_until: Timestamp,
    /// Reason for the ban.
    pub reason: BanReason,
}

/// Details for banning a peer.
pub struct BanDetails {
    /// Duration of the ban in seconds.
    pub duration_secs: u64,
    /// Reason for the ban.
    pub reason: BanReason,
}

impl BanDetails {
    /// Create new ban details.
    pub fn new(duration_secs: u64, reason: BanReason) -> Self {
        Self {
            duration_secs,
            reason,
        }
    }
}

/// Statistics about the routing table state
///
/// Reference: SPEC-01 Section 3.1
#[derive(Debug, Clone, Default)]
pub struct RoutingTableStats {
    /// Total number of verified peers in buckets
    pub total_peers: usize,
    /// Number of buckets with at least one peer
    pub buckets_used: usize,
    /// Number of currently banned peers
    pub banned_count: usize,
    /// Age of the oldest peer in seconds
    pub oldest_peer_age_seconds: u64,
    /// Current count of peers awaiting verification (V2.3)
    pub pending_verification_count: usize,
    /// Maximum allowed pending peers (V2.3)
    pub max_pending_peers: usize,
}
