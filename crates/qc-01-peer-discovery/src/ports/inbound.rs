//! # Driving Ports (Inbound API)
//!
//! These are the public APIs this subsystem exposes to the application node.
//!
//! Per SPEC-01-PEER-DISCOVERY.md Section 3.1

use crate::domain::{BanReason, NodeId, PeerDiscoveryError, PeerInfo, RoutingTableStats};

/// Primary API for interacting with the peer discovery subsystem.
///
/// This trait defines the driving port (inbound API) that consumers use
/// to interact with peer discovery functionality.
///
/// # Security
///
/// All methods enforce the security invariants from SPEC-01:
/// - INVARIANT-9: Bounded staging (Memory Bomb Defense)
/// - INVARIANT-10: Eviction-on-Failure (Eclipse Attack Defense)
///
/// # Example
///
/// ```rust,ignore
/// use qc_01_peer_discovery::ports::PeerDiscoveryApi;
///
/// fn discover_peers<T: PeerDiscoveryApi>(api: &T, target: NodeId) {
///     let closest = api.find_closest_peers(target, 20);
///     println!("Found {} peers", closest.len());
/// }
/// ```
pub trait PeerDiscoveryApi {
    /// Find the k closest peers to a target ID.
    ///
    /// Used for iterative node lookups in the Kademlia DHT.
    ///
    /// # Arguments
    ///
    /// * `target_id` - The NodeId to search for
    /// * `count` - Maximum number of peers to return (typically k=20)
    ///
    /// # Returns
    ///
    /// A vector of peers sorted by XOR distance to the target.
    fn find_closest_peers(&self, target_id: NodeId, count: usize) -> Vec<PeerInfo>;

    /// Add a newly discovered peer to the staging area for verification.
    ///
    /// # Security (Bounded Staging - V2.3)
    ///
    /// This function enforces INVARIANT-9 (Memory Bomb Defense):
    /// - If `pending_verification.len() >= max_pending_peers`, returns `Err(StagingAreaFull)`
    /// - Peer is NOT added; request is immediately dropped (Tail Drop Strategy)
    /// - No eviction of existing pending peers (prioritizes honest work)
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if staged for verification
    /// - `Ok(false)` if rejected (banned, subnet limit, etc.)
    /// - `Err(StagingAreaFull)` if staging capacity reached
    /// - `Err(SelfConnection)` if attempting to add local node
    fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError>;

    /// Get a random selection of peers (for gossip protocols).
    ///
    /// Used by Subsystem 5 (Block Propagation) for broadcast.
    ///
    /// # Arguments
    ///
    /// * `count` - Maximum number of peers to return
    ///
    /// # Returns
    ///
    /// A vector of randomly selected peers from the routing table.
    /// May return fewer than `count` if not enough peers available.
    fn get_random_peers(&self, count: usize) -> Vec<PeerInfo>;

    /// Manually ban a peer for a duration.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The peer to ban
    /// * `duration_seconds` - How long to ban (0 = permanent)
    /// * `reason` - Why the peer is being banned
    ///
    /// # Note
    ///
    /// Per SPEC-01 Section 2.2, `BanReason::InvalidSignature` is intentionally
    /// excluded to prevent IP spoofing attacks. Invalid signatures result in
    /// silent drops, not bans.
    fn ban_peer(
        &mut self,
        node_id: NodeId,
        duration_seconds: u64,
        reason: BanReason,
    ) -> Result<(), PeerDiscoveryError>;

    /// Check if a peer is currently banned.
    ///
    /// # Returns
    ///
    /// `true` if the peer is banned and the ban has not expired.
    fn is_banned(&self, node_id: NodeId) -> bool;

    /// Update peer's last-seen timestamp (keep-alive).
    ///
    /// Called when we receive valid communication from a peer.
    /// This moves the peer toward the "most recently seen" position
    /// in its k-bucket, making it less likely to be evicted.
    fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError>;

    /// Remove a peer from routing table.
    ///
    /// Called due to timeout, network error, or explicit removal.
    fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError>;

    /// Get current routing table statistics.
    ///
    /// # Returns
    ///
    /// Statistics including peer counts, staging area status, and health metrics.
    fn get_stats(&self) -> RoutingTableStats;
}
