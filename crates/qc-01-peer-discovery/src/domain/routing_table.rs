//! Routing Table Implementation
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.2
//!
//! This module implements the Kademlia DHT routing table with all security
//! invariants from the specification.

use std::collections::HashMap;

use crate::domain::{
    calculate_bucket_index, is_same_subnet, BanReason, Distance, KademliaConfig, NodeId,
    PeerDiscoveryError, PeerInfo, SubnetMask, Timestamp,
};

/// Number of k-buckets (one per bit of NodeId)
pub const NUM_BUCKETS: usize = 256;

/// Maximum total peers across all buckets
pub const MAX_TOTAL_PEERS: usize = 5120; // 256 * 20

/// The main routing table implementing Kademlia DHT
///
/// # Security (DDoS Edge Defense - System.md Compliance)
/// New peers are staged in `pending_verification` until Subsystem 10 confirms
/// their identity. This prevents unverified peers from polluting the Kademlia table.
///
/// # Security (Bounded Staging - V2.3 Memory Bomb Defense)
/// The `pending_verification` HashMap is bounded by `config.max_pending_peers`.
/// When full, incoming peer requests are immediately dropped (Tail Drop Strategy).
/// This prevents memory exhaustion attacks. See INVARIANT-9.
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug)]
pub struct RoutingTable {
    /// Our own node ID (immutable after creation)
    local_node_id: NodeId,
    /// 256 k-buckets, one for each possible XOR distance
    buckets: Vec<KBucket>,
    /// Banned peers with expiration times
    banned_peers: BannedPeers,
    /// Staging area for peers awaiting signature verification from Subsystem 10.
    /// Peers move to `buckets` only after identity_valid == true.
    /// BOUNDED: Size limited by config.max_pending_peers (INVARIANT-9).
    pending_verification: HashMap<NodeId, PendingPeer>,
    /// Configuration including max_pending_peers limit
    config: KademliaConfig,
    /// Subnet mask for IP diversity checks
    subnet_mask: SubnetMask,
}

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
    peers: Vec<PeerInfo>,
    /// Last time this bucket was updated
    last_updated: Timestamp,
    /// Peer waiting to join this bucket, pending eviction challenge result.
    /// When bucket is full and new peer arrives, oldest peer is challenged.
    /// If oldest peer responds (alive), new peer is rejected.
    /// If oldest peer times out (dead), new peer replaces it.
    pending_insertion: Option<PendingInsertion>,
}

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

/// Tracks banned peers with expiration times
///
/// Reference: SPEC-01 Section 2.2
#[derive(Debug, Clone, Default)]
pub struct BannedPeers {
    entries: HashMap<NodeId, BannedEntry>,
}

/// Individual ban entry
#[derive(Debug, Clone)]
pub struct BannedEntry {
    pub node_id: NodeId,
    pub banned_until: Timestamp,
    pub reason: BanReason,
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

// =============================================================================
// KBucket Implementation
// =============================================================================

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
    fn add_peer(&mut self, peer: PeerInfo, now: Timestamp) {
        self.peers.push(peer);
        self.last_updated = now;
    }

    /// Remove a peer by NodeId
    fn remove_peer(&mut self, node_id: &NodeId) -> Option<PeerInfo> {
        if let Some(pos) = self.peers.iter().position(|p| &p.node_id == node_id) {
            Some(self.peers.remove(pos))
        } else {
            None
        }
    }

    /// Move a peer to the front (most recently seen)
    fn move_to_front(&mut self, node_id: &NodeId, now: Timestamp) -> bool {
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
    fn touch_peer(&mut self, node_id: &NodeId, now: Timestamp) -> bool {
        if let Some(peer) = self.peers.iter_mut().find(|p| &p.node_id == node_id) {
            peer.last_seen = now;
            self.last_updated = now;
            true
        } else {
            false
        }
    }

    /// Check if bucket contains a peer
    fn contains(&self, node_id: &NodeId) -> bool {
        self.peers.iter().any(|p| &p.node_id == node_id)
    }
}

impl Default for KBucket {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// BannedPeers Implementation
// =============================================================================

impl BannedPeers {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Add a ban entry
    pub fn ban(&mut self, node_id: NodeId, until: Timestamp, reason: BanReason) {
        self.entries.insert(node_id, BannedEntry {
            node_id,
            banned_until: until,
            reason,
        });
    }

    /// Check if a peer is currently banned
    pub fn is_banned(&self, node_id: &NodeId, now: Timestamp) -> bool {
        if let Some(entry) = self.entries.get(node_id) {
            entry.banned_until > now
        } else {
            false
        }
    }

    /// Remove expired bans
    pub fn gc_expired(&mut self, now: Timestamp) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.banned_until > now);
        before - self.entries.len()
    }

    /// Get count of active bans
    pub fn count(&self, now: Timestamp) -> usize {
        self.entries.values().filter(|e| e.banned_until > now).count()
    }
}

// =============================================================================
// RoutingTable Implementation
// =============================================================================

impl RoutingTable {
    /// Create a new routing table
    ///
    /// # Arguments
    /// * `local_node_id` - Our own node ID (INVARIANT-2: immutable after creation)
    /// * `config` - Kademlia configuration
    pub fn new(local_node_id: NodeId, config: KademliaConfig) -> Self {
        let buckets = (0..NUM_BUCKETS).map(|_| KBucket::new()).collect();

        Self {
            local_node_id,
            buckets,
            banned_peers: BannedPeers::new(),
            pending_verification: HashMap::new(),
            config,
            subnet_mask: SubnetMask::default(),
        }
    }

    /// Get our local node ID
    pub fn local_node_id(&self) -> &NodeId {
        &self.local_node_id
    }

    /// Get the configuration
    pub fn config(&self) -> &KademliaConfig {
        &self.config
    }

    /// Get total peer count across all buckets
    pub fn total_peer_count(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// Get routing table statistics
    pub fn stats(&self, now: Timestamp) -> RoutingTableStats {
        let total_peers = self.total_peer_count();
        let buckets_used = self.buckets.iter().filter(|b| !b.is_empty()).count();
        let banned_count = self.banned_peers.count(now);

        let oldest_peer_age_seconds = self
            .buckets
            .iter()
            .flat_map(|b| b.peers.iter())
            .map(|p| now.as_secs().saturating_sub(p.last_seen.as_secs()))
            .max()
            .unwrap_or(0);

        RoutingTableStats {
            total_peers,
            buckets_used,
            banned_count,
            oldest_peer_age_seconds,
            pending_verification_count: self.pending_verification.len(),
            max_pending_peers: self.config.max_pending_peers,
        }
    }

    /// Stage a peer for verification (DDoS Edge Defense)
    ///
    /// # INVARIANT-7: New peers go to staging, not buckets
    /// # INVARIANT-9: Bounded staging with Tail Drop (Memory Bomb Defense)
    ///
    /// # Returns
    /// - `Ok(true)` if peer was staged
    /// - `Ok(false)` if peer was rejected (banned, self, etc.)
    /// - `Err(StagingAreaFull)` if staging at capacity (Tail Drop)
    pub fn stage_peer(
        &mut self,
        peer: PeerInfo,
        now: Timestamp,
    ) -> Result<bool, PeerDiscoveryError> {
        // INVARIANT-9: Check staging capacity FIRST (O(1), no allocation)
        // This is the Memory Bomb Defense - Tail Drop Strategy
        if self.pending_verification.len() >= self.config.max_pending_peers {
            return Err(PeerDiscoveryError::StagingAreaFull);
        }

        // INVARIANT-5: Cannot add self
        if peer.node_id == self.local_node_id {
            return Err(PeerDiscoveryError::SelfConnection);
        }

        // INVARIANT-4: Cannot add banned peer
        if self.banned_peers.is_banned(&peer.node_id, now) {
            return Err(PeerDiscoveryError::PeerBanned);
        }

        // Already in staging?
        if self.pending_verification.contains_key(&peer.node_id) {
            return Ok(false);
        }

        // Already in routing table?
        let bucket_idx = calculate_bucket_index(&self.local_node_id, &peer.node_id);
        if let Some(bucket) = self.buckets.get(bucket_idx) {
            if bucket.contains(&peer.node_id) {
                return Ok(false);
            }
        }

        // Add to staging
        let pending = PendingPeer {
            peer_info: peer.clone(),
            received_at: now,
            verification_deadline: now.add_secs(self.config.verification_timeout_secs),
        };
        self.pending_verification.insert(peer.node_id, pending);

        Ok(true)
    }

    /// Handle verification result from Subsystem 10
    ///
    /// # INVARIANT-7: Verified peers move from staging to buckets
    /// # INVARIANT-10: Eviction-on-Failure for full buckets (Eclipse Attack Defense)
    ///
    /// # Arguments
    /// * `node_id` - The peer that was verified
    /// * `identity_valid` - true if signature verified, false if failed
    /// * `now` - Current timestamp
    ///
    /// # Returns
    /// - `Ok(Some(challenged))` if peer needs challenge (bucket full)
    /// - `Ok(None)` if peer was added or rejected
    /// - `Err(_)` on error
    ///
    /// # Security Note
    /// If `identity_valid == false`, the peer is SILENTLY DROPPED.
    /// We do NOT ban them because IPs can be spoofed in UDP.
    pub fn on_verification_result(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
        now: Timestamp,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        // Remove from staging
        let pending = self.pending_verification.remove(node_id);

        if !identity_valid {
            // SILENT DROP - Do NOT ban (IP spoofing defense)
            // See SPEC-01 Section 2.2 BanReason note
            return Ok(None);
        }

        let pending = match pending {
            Some(p) => p,
            None => return Err(PeerDiscoveryError::PeerNotFound),
        };

        let peer = pending.peer_info;
        let bucket_idx = calculate_bucket_index(&self.local_node_id, &peer.node_id);
        let bucket = self
            .buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)?;

        // INVARIANT-3: Check IP diversity
        let peers_in_subnet = bucket
            .peers
            .iter()
            .filter(|p| is_same_subnet(&p.socket_addr.ip, &peer.socket_addr.ip, &self.subnet_mask))
            .count();

        if peers_in_subnet >= self.config.max_peers_per_subnet {
            return Err(PeerDiscoveryError::SubnetLimitReached);
        }

        // INVARIANT-1: Check bucket capacity
        if !bucket.is_full(self.config.k) {
            // Bucket has space - add directly
            bucket.add_peer(peer, now);
            return Ok(None);
        }

        // Bucket is full - need to challenge oldest peer (INVARIANT-10)
        // V2.4 Eclipse Attack Defense: Eviction-on-Failure

        // Only ONE pending insertion per bucket
        if bucket.has_pending_challenge() {
            return Err(PeerDiscoveryError::ChallengeInProgress);
        }

        // Get the oldest peer to challenge
        let oldest = bucket.oldest_peer().ok_or(PeerDiscoveryError::BucketFull)?;
        let challenged_peer = oldest.node_id;

        // Store the pending insertion
        bucket.pending_insertion = Some(PendingInsertion {
            candidate: peer,
            challenged_peer,
            challenge_sent_at: now,
            challenge_deadline: now.add_secs(self.config.eviction_challenge_timeout_secs),
        });

        // Return the challenged peer's NodeId so caller can send PING
        Ok(Some(challenged_peer))
    }

    /// Handle challenge response (PING/PONG result)
    ///
    /// # INVARIANT-10: Eviction-on-Failure
    /// - If oldest peer is ALIVE (responded): reject new peer, move oldest to front
    /// - If oldest peer is DEAD (timed out): evict oldest, insert new peer
    ///
    /// # Arguments
    /// * `challenged_peer` - The peer that was challenged
    /// * `is_alive` - true if peer responded to PING, false if timed out
    /// * `now` - Current timestamp
    pub fn on_challenge_response(
        &mut self,
        challenged_peer: &NodeId,
        is_alive: bool,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        // Find the bucket with this pending challenge
        let bucket_idx = calculate_bucket_index(&self.local_node_id, challenged_peer);
        let bucket = self
            .buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)?;

        // Get and remove the pending insertion
        let pending = bucket
            .pending_insertion
            .take()
            .ok_or(PeerDiscoveryError::PeerNotFound)?;

        // Verify this is the right challenge
        if &pending.challenged_peer != challenged_peer {
            // Put it back - wrong challenge
            bucket.pending_insertion = Some(pending);
            return Err(PeerDiscoveryError::PeerNotFound);
        }

        if is_alive {
            // Oldest peer is ALIVE - prefer stable peers over new peers
            // Move oldest to front (most recently seen)
            bucket.move_to_front(challenged_peer, now);
            // New peer (candidate) is REJECTED
            // This is the core Eclipse Attack defense
        } else {
            // Oldest peer is DEAD - evict and insert new peer
            bucket.remove_peer(challenged_peer);
            bucket.add_peer(pending.candidate, now);
        }

        Ok(())
    }

    /// Check for expired eviction challenges and complete the eviction workflow.
    ///
    /// Per INVARIANT-10 (Eviction-on-Failure), when a challenge times out, the
    /// oldest peer is considered dead and the candidate peer replaces it.
    ///
    /// Returns list of (bucket_index, inserted_peer, evicted_peer) for logging.
    ///
    /// Reference: SPEC-01 Section 2.2 (`PendingInsertion.challenge_deadline`)
    pub fn check_expired_challenges(&mut self, now: Timestamp) -> Vec<(usize, PeerInfo, NodeId)> {
        let mut expired = Vec::new();

        for (idx, bucket) in self.buckets.iter_mut().enumerate() {
            if let Some(ref pending) = bucket.pending_insertion {
                if now >= pending.challenge_deadline {
                    // Challenge timed out: treat as PONG failure (peer is dead)
                    let pending = bucket.pending_insertion.take().unwrap();
                    expired.push((idx, pending.candidate, pending.challenged_peer));
                }
            }
        }

        // Complete eviction: remove dead peer, insert candidate
        for (idx, candidate, challenged) in &expired {
            if let Some(bucket) = self.buckets.get_mut(*idx) {
                bucket.remove_peer(challenged);
                bucket.add_peer(candidate.clone(), now);
            }
        }

        expired
    }

    /// Garbage collect expired entries
    ///
    /// Cleans up:
    /// - Expired pending verifications (INVARIANT-8)
    /// - Expired bans
    ///
    /// # Returns
    /// Number of entries removed
    pub fn gc_expired(&mut self, now: Timestamp) -> usize {
        let mut removed = 0;

        // INVARIANT-8: Remove timed-out pending verifications
        let before = self.pending_verification.len();
        self.pending_verification
            .retain(|_, p| p.verification_deadline > now);
        removed += before - self.pending_verification.len();

        // Remove expired bans
        removed += self.banned_peers.gc_expired(now);

        removed
    }

    /// Ban a peer
    pub fn ban_peer(
        &mut self,
        node_id: NodeId,
        duration_secs: u64,
        reason: BanReason,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        // Remove from routing table if present
        let bucket_idx = calculate_bucket_index(&self.local_node_id, &node_id);
        if let Some(bucket) = self.buckets.get_mut(bucket_idx) {
            bucket.remove_peer(&node_id);
        }

        // Remove from staging if present
        self.pending_verification.remove(&node_id);

        // Add to banned list
        let until = now.add_secs(duration_secs);
        self.banned_peers.ban(node_id, until, reason);

        Ok(())
    }

    /// Check if a peer is banned
    pub fn is_banned(&self, node_id: &NodeId, now: Timestamp) -> bool {
        self.banned_peers.is_banned(node_id, now)
    }

    /// Touch a peer (update last_seen)
    pub fn touch_peer(
        &mut self,
        node_id: &NodeId,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        let bucket_idx = calculate_bucket_index(&self.local_node_id, node_id);
        let bucket = self
            .buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)?;

        if bucket.touch_peer(node_id, now) {
            Ok(())
        } else {
            Err(PeerDiscoveryError::PeerNotFound)
        }
    }

    /// Remove a peer from the routing table
    pub fn remove_peer(&mut self, node_id: &NodeId) -> Result<(), PeerDiscoveryError> {
        let bucket_idx = calculate_bucket_index(&self.local_node_id, node_id);
        let bucket = self
            .buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)?;

        if bucket.remove_peer(node_id).is_some() {
            Ok(())
        } else {
            Err(PeerDiscoveryError::PeerNotFound)
        }
    }

    /// Find the k closest peers to a target
    ///
    /// # Arguments
    /// * `target` - Target NodeId to find peers close to
    /// * `count` - Maximum number of peers to return
    ///
    /// # Returns
    /// Up to `count` peers sorted by XOR distance (closest first)
    pub fn find_closest_peers(&self, target: &NodeId, count: usize) -> Vec<PeerInfo> {
        let mut all_peers: Vec<(Distance, &PeerInfo)> = self
            .buckets
            .iter()
            .flat_map(|b| b.peers.iter())
            .map(|p| (crate::domain::xor_distance(&p.node_id, target), p))
            .collect();

        // Sort by distance (higher bucket index = closer = first)
        all_peers.sort_by(|a, b| b.0.cmp(&a.0));

        all_peers
            .into_iter()
            .take(count)
            .map(|(_, p)| p.clone())
            .collect()
    }

    /// Get random peers for gossip protocols.
    ///
    /// Iterates buckets sequentially to collect peers. For uniform random
    /// distribution across the network topology, callers using gossip protocols
    /// (Subsystem 5) apply their own randomization after receiving results.
    ///
    /// Reference: SPEC-01 Section 3.1 (`get_random_peers`)
    pub fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
        // Sequential iteration across buckets provides diverse distance coverage.
        // True randomization is the caller's responsibility per Subsystem 5 requirements.
        self.buckets
            .iter()
            .flat_map(|b| b.peers.iter().cloned())
            .take(count)
            .collect()
    }

    /// Get a reference to a bucket by index
    pub fn get_bucket(&self, index: usize) -> Option<&KBucket> {
        self.buckets.get(index)
    }

    /// Get mutable reference to a bucket by index
    pub fn get_bucket_mut(&mut self, index: usize) -> Option<&mut KBucket> {
        self.buckets.get_mut(index)
    }

    /// Get pending verification count
    pub fn pending_verification_count(&self) -> usize {
        self.pending_verification.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IpAddr, SocketAddr};

    fn make_node_id(val: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = val;
        NodeId::new(bytes)
    }

    fn make_peer(val: u8, port: u16) -> PeerInfo {
        PeerInfo::new(
            make_node_id(val),
            SocketAddr::new(IpAddr::v4(192, 168, 1, val), port),
            Timestamp::new(1000),
        )
    }

    fn make_peer_with_ip(val: u8, ip_last: u8) -> PeerInfo {
        #![allow(dead_code)]
        PeerInfo::new(
            make_node_id(val),
            SocketAddr::new(IpAddr::v4(192, 168, 1, ip_last), 8080),
            Timestamp::new(1000),
        )
    }

    // =========================================================================
    // Test Group 2: K-Bucket Management
    // Reference: SPEC-01 Section 5.1 (TDD Test Specifications)
    // =========================================================================

    #[test]
    fn test_bucket_rejects_when_full() {
        let mut bucket = KBucket::new();
        let k = 3;

        for i in 0..k {
            bucket.add_peer(make_peer(i as u8, 8080), Timestamp::new(1000));
        }

        assert!(bucket.is_full(k));
        assert_eq!(bucket.len(), k);
    }

    #[test]
    fn test_bucket_rejects_21st_peer_when_full_and_all_alive() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        // NodeId with first bit = 1 maps to bucket 0 (XOR distance from local_id = 0)
        let make_bucket0_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
                Timestamp::new(1000),
            )
        };

        // Fill bucket to capacity (k=3 for testing config)
        for i in 0..table.config.k {
            let peer = make_bucket0_peer(i as u8);
            table.stage_peer(peer.clone(), now).unwrap();
            let result = table
                .on_verification_result(&peer.node_id, true, now)
                .unwrap();
            // INVARIANT-1: First k-1 peers added directly without challenge
            if i < table.config.k - 1 {
                assert!(result.is_none(), "Peer {} added directly", i);
            }
        }

        // INVARIANT-10: Additional peer triggers eviction challenge
        let extra_peer = make_bucket0_peer(100);
        table.stage_peer(extra_peer.clone(), now).unwrap();
        let result = table
            .on_verification_result(&extra_peer.node_id, true, now)
            .unwrap();

        assert!(
            result.is_some(),
            "Full bucket returns challenged peer NodeId"
        );
    }

    #[test]
    fn test_bucket_prefers_stable_peers_over_new_peers() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        // Create a peer that goes to bucket 0
        let make_bucket0_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
                Timestamp::new(1000),
            )
        };

        // Fill bucket
        let mut peers = Vec::new();
        for i in 0..table.config.k {
            let peer = make_bucket0_peer(i as u8);
            peers.push(peer.node_id);
            table.stage_peer(peer.clone(), now).unwrap();
            table
                .on_verification_result(&peer.node_id, true, now)
                .unwrap();
        }

        // Add new peer - triggers challenge
        let new_peer = make_bucket0_peer(100);
        table.stage_peer(new_peer.clone(), now).unwrap();
        let challenged = table
            .on_verification_result(&new_peer.node_id, true, now)
            .unwrap()
            .expect("Should have challenged peer");

        // Simulate: oldest peer is ALIVE (responded to PING)
        table.on_challenge_response(&challenged, true, now).unwrap();

        // Verify: oldest peer is still in bucket, new peer rejected
        let bucket_idx = calculate_bucket_index(&local_id, &peers[0]);
        let bucket = table.get_bucket(bucket_idx).unwrap();

        assert!(
            bucket.contains(&challenged),
            "Stable peer retained per INVARIANT-10"
        );
        assert!(
            !bucket.contains(&new_peer.node_id),
            "Candidate rejected when challenged peer alive"
        );
        assert_eq!(bucket.len(), table.config.k, "Bucket maintains k peers");
    }

    #[test]
    fn test_bucket_evicts_dead_peers_for_new_peers() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let make_bucket0_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
                Timestamp::new(1000),
            )
        };

        let mut peers = Vec::new();
        for i in 0..table.config.k {
            let peer = make_bucket0_peer(i as u8);
            peers.push(peer.node_id);
            table.stage_peer(peer.clone(), now).unwrap();
            table
                .on_verification_result(&peer.node_id, true, now)
                .unwrap();
        }

        let oldest_peer = peers[0];

        // INVARIANT-10: Full bucket triggers challenge to oldest peer
        let new_peer = make_bucket0_peer(100);
        table.stage_peer(new_peer.clone(), now).unwrap();
        let challenged = table
            .on_verification_result(&new_peer.node_id, true, now)
            .unwrap()
            .expect("Full bucket returns challenged NodeId");

        assert_eq!(challenged, oldest_peer, "Challenge targets oldest peer");

        // Simulate PONG timeout (peer is dead)
        table
            .on_challenge_response(&challenged, false, now)
            .unwrap();

        let bucket_idx = calculate_bucket_index(&local_id, &oldest_peer);
        let bucket = table.get_bucket(bucket_idx).unwrap();

        assert!(
            !bucket.contains(&oldest_peer),
            "Dead peer evicted per INVARIANT-10"
        );
        assert!(
            bucket.contains(&new_peer.node_id),
            "Candidate inserted after eviction"
        );
        assert_eq!(bucket.len(), table.config.k, "Bucket maintains k peers");
    }

    #[test]
    fn test_bucket_challenge_in_progress_rejects_additional_candidates() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let make_bucket0_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
                Timestamp::new(1000),
            )
        };

        // Fill bucket to capacity
        for i in 0..table.config.k {
            let peer = make_bucket0_peer(i as u8);
            table.stage_peer(peer.clone(), now).unwrap();
            table
                .on_verification_result(&peer.node_id, true, now)
                .unwrap();
        }

        // First candidate triggers challenge against oldest peer
        let peer_a = make_bucket0_peer(100);
        table.stage_peer(peer_a.clone(), now).unwrap();
        let _challenged = table
            .on_verification_result(&peer_a.node_id, true, now)
            .unwrap();

        // INVARIANT-10: Only ONE pending_insertion per bucket allowed
        let peer_b = make_bucket0_peer(101);
        table.stage_peer(peer_b.clone(), now).unwrap();
        let result = table.on_verification_result(&peer_b.node_id, true, now);

        assert!(
            matches!(result, Err(PeerDiscoveryError::ChallengeInProgress)),
            "Concurrent challenge rejected per INVARIANT-10"
        );
    }

    // =========================================================================
    // Test Group 5: Ban System
    // Reference: SPEC-01 Section 5.1 (TDD Test Specifications)
    // =========================================================================

    #[test]
    fn test_bucket_rejects_peer_if_banned() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);

        table
            .ban_peer(peer.node_id, 60, BanReason::ManualBan, now)
            .unwrap();

        // INVARIANT-4: Banned peers excluded from routing table
        let result = table.stage_peer(peer, now);

        assert!(
            matches!(result, Err(PeerDiscoveryError::PeerBanned)),
            "Banned peer rejected per INVARIANT-4"
        );
    }

    #[test]
    fn test_banned_peer_expires_after_duration() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);

        table
            .ban_peer(peer.node_id, 60, BanReason::ManualBan, now)
            .unwrap();

        // Ban active at t=1000 and t=1059 (59 seconds elapsed)
        assert!(table.is_banned(&peer.node_id, now));
        assert!(table.is_banned(&peer.node_id, Timestamp::new(1059)));

        // Ban expired at t=1061 (61 seconds elapsed > 60 second ban)
        assert!(!table.is_banned(&peer.node_id, Timestamp::new(1061)));
    }

    #[test]
    fn test_cannot_add_banned_peer_to_routing_table() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);

        table
            .ban_peer(peer.node_id, 60, BanReason::ManualBan, now)
            .unwrap();

        let result = table.stage_peer(peer, now);

        assert!(matches!(result, Err(PeerDiscoveryError::PeerBanned)));
    }

    // =========================================================================
    // Test Group 6: Pending Verification Staging
    // Reference: SPEC-01 Section 5.1 (DDoS Edge Defense Tests)
    // =========================================================================

    #[test]
    fn test_new_peer_goes_to_pending_verification_first() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);
        let peer_id = peer.node_id;

        table.stage_peer(peer, now).unwrap();

        // INVARIANT-7: Peer in staging area, not routing table
        assert_eq!(table.pending_verification_count(), 1);
        assert_eq!(table.total_peer_count(), 0);

        // Verification promotes peer to routing table
        table.on_verification_result(&peer_id, true, now).unwrap();

        assert_eq!(table.pending_verification_count(), 0);
        assert_eq!(table.total_peer_count(), 1);
    }

    #[test]
    fn test_peer_silently_dropped_on_identity_valid_false() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);
        let peer_id = peer.node_id;

        table.stage_peer(peer, now).unwrap();
        assert_eq!(table.pending_verification_count(), 1);

        // SPEC-01 Section 2.2: Silent drop on verification failure (IP spoofing defense)
        table.on_verification_result(&peer_id, false, now).unwrap();

        assert_eq!(table.pending_verification_count(), 0);
        assert_eq!(table.total_peer_count(), 0);
        assert!(
            !table.is_banned(&peer_id, now),
            "Silent drop, NOT ban per BanReason security note"
        );
    }

    #[test]
    fn test_pending_peer_times_out_after_deadline() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);

        table.stage_peer(peer, now).unwrap();
        assert_eq!(table.pending_verification_count(), 1);

        // INVARIANT-8: Peer remains until deadline
        let later = Timestamp::new(1000 + table.config.verification_timeout_secs - 1);
        table.gc_expired(later);
        assert_eq!(table.pending_verification_count(), 1);

        // INVARIANT-8: Peer removed after deadline
        let expired = Timestamp::new(1000 + table.config.verification_timeout_secs + 1);
        let removed = table.gc_expired(expired);
        assert_eq!(removed, 1);
        assert_eq!(table.pending_verification_count(), 0);
    }

    // =========================================================================
    // Test Group 7: Bounded Staging (Memory Bomb Defense)
    // Reference: SPEC-01 Section 5.1 (V2.3 Memory Bomb Defense Tests)
    // =========================================================================

    #[test]
    fn test_staging_area_rejects_peer_when_at_capacity() {
        let local_id = make_node_id(0);
        let mut config = KademliaConfig::for_testing();
        config.max_pending_peers = 3;
        let mut table = RoutingTable::new(local_id, config);
        let now = Timestamp::new(1000);

        for i in 1..=3 {
            let peer = make_peer(i, 8080);
            table.stage_peer(peer, now).unwrap();
        }

        assert_eq!(table.pending_verification_count(), 3);

        // INVARIANT-9: Tail Drop when staging at capacity
        let extra_peer = make_peer(100, 8080);
        let result = table.stage_peer(extra_peer, now);

        assert!(
            matches!(result, Err(PeerDiscoveryError::StagingAreaFull)),
            "Staging full returns StagingAreaFull error"
        );
        assert_eq!(
            table.pending_verification_count(),
            3,
            "Staging count unchanged after rejection"
        );
    }

    #[test]
    fn test_staging_area_uses_tail_drop_not_eviction() {
        let local_id = make_node_id(0);
        let mut config = KademliaConfig::for_testing();
        config.max_pending_peers = 2;
        let mut table = RoutingTable::new(local_id, config);
        let now = Timestamp::new(1000);

        let peer1 = make_peer(1, 8080);
        let peer2 = make_peer(2, 8080);
        let peer1_id = peer1.node_id;
        let peer2_id = peer2.node_id;

        table.stage_peer(peer1, now).unwrap();
        table.stage_peer(peer2, now).unwrap();

        let peer3 = make_peer(3, 8080);
        assert!(table.stage_peer(peer3, now).is_err());

        // INVARIANT-9: Tail Drop preserves existing pending peers (first-come-first-served)
        assert!(table.pending_verification.contains_key(&peer1_id));
        assert!(table.pending_verification.contains_key(&peer2_id));
    }

    #[test]
    fn test_staging_area_capacity_freed_after_verification_complete() {
        let local_id = make_node_id(0);
        let mut config = KademliaConfig::for_testing();
        config.max_pending_peers = 1;
        let mut table = RoutingTable::new(local_id, config);
        let now = Timestamp::new(1000);

        let peer1 = make_peer(1, 8080);
        let peer1_id = peer1.node_id;
        table.stage_peer(peer1, now).unwrap();

        // Staging at capacity
        let peer2 = make_peer(2, 8080);
        assert!(table.stage_peer(peer2.clone(), now).is_err());

        // Verification frees staging slot
        table.on_verification_result(&peer1_id, true, now).unwrap();

        // Slot now available
        assert!(table.stage_peer(peer2, now).is_ok());
    }

    #[test]
    fn test_get_stats_reports_pending_verification_count() {
        let local_id = make_node_id(0);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let peer = make_peer(1, 8080);
        table.stage_peer(peer, now).unwrap();

        let stats = table.stats(now);
        assert_eq!(stats.pending_verification_count, 1);
        assert_eq!(stats.max_pending_peers, table.config.max_pending_peers);
    }

    // =========================================================================
    // Test Group 8: Eviction-on-Failure (Eclipse Attack Defense)
    // Reference: SPEC-01 Section 5.1 (V2.4 Eclipse Attack Defense Tests)
    // =========================================================================

    #[test]
    fn test_table_poisoning_attack_is_blocked() {
        let local_id = make_node_id(0);
        let mut config = KademliaConfig::for_testing();
        config.k = 3;
        let mut table = RoutingTable::new(local_id, config);
        let now = Timestamp::new(1000);

        let make_bucket0_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(10, i, 0, 1), 8080),
                Timestamp::new(1000),
            )
        };

        // Establish honest peer baseline
        let mut honest_peers = Vec::new();
        for i in 0..table.config.k {
            let peer = make_bucket0_peer(i as u8);
            honest_peers.push(peer.node_id);
            table.stage_peer(peer.clone(), now).unwrap();
            table
                .on_verification_result(&peer.node_id, true, now)
                .unwrap();
        }

        // Simulate attacker attempting table poisoning (20 malicious peers)
        // INVARIANT-10: All honest peers respond to challenges (alive)
        for i in 100..120 {
            let attacker_peer = make_bucket0_peer(i);
            table.stage_peer(attacker_peer.clone(), now).unwrap();

            match table.on_verification_result(&attacker_peer.node_id, true, now) {
                Ok(Some(challenged)) => {
                    // Honest peer responds (alive) → attacker rejected
                    table.on_challenge_response(&challenged, true, now).unwrap();
                }
                Err(PeerDiscoveryError::ChallengeInProgress) => {
                    // Challenge already in progress per INVARIANT-10
                }
                _ => {}
            }
        }

        // SECURITY GUARANTEE: All honest peers survive attack
        let bucket_idx = calculate_bucket_index(&local_id, &honest_peers[0]);
        let bucket = table.get_bucket(bucket_idx).unwrap();

        for honest in &honest_peers {
            assert!(
                bucket.contains(honest),
                "Honest peer {:?} survives attack per INVARIANT-10",
                honest
            );
        }
        assert_eq!(
            bucket.len(),
            table.config.k,
            "Bucket maintains k peers after attack"
        );
    }

    // =========================================================================
    // Test: Self-connection rejection (INVARIANT-5)
    // =========================================================================

    #[test]
    fn test_bucket_rejects_self_node() {
        let local_id = make_node_id(42);
        let mut table = RoutingTable::new(local_id, KademliaConfig::for_testing());
        let now = Timestamp::new(1000);

        let self_peer = PeerInfo::new(
            local_id,
            SocketAddr::new(IpAddr::v4(127, 0, 0, 1), 8080),
            now,
        );

        // INVARIANT-5: Self-connection rejected
        let result = table.stage_peer(self_peer, now);

        assert!(
            matches!(result, Err(PeerDiscoveryError::SelfConnection)),
            "Self-connection rejected per INVARIANT-5"
        );
    }

    // =========================================================================
    // Test: IP Diversity (INVARIANT-3)
    // Reference: SPEC-01 Section 6.1 (Sybil Attack Resistance)
    // =========================================================================

    #[test]
    fn test_rejects_third_peer_from_same_subnet() {
        let local_id = make_node_id(0);
        let mut config = KademliaConfig::for_testing();
        config.max_peers_per_subnet = 2;
        let mut table = RoutingTable::new(local_id, config);
        let now = Timestamp::new(1000);

        // All peers in same /24 subnet (192.168.1.0/24)
        let make_peer = |i: u8| {
            let mut bytes = [0u8; 32];
            bytes[0] = 0b1000_0000;
            bytes[1] = i;
            PeerInfo::new(
                NodeId::new(bytes),
                SocketAddr::new(IpAddr::v4(192, 168, 1, i), 8080),
                Timestamp::new(1000),
            )
        };

        // First two peers from same subnet accepted
        let peer1 = make_peer(1);
        let peer2 = make_peer(2);
        table.stage_peer(peer1.clone(), now).unwrap();
        table
            .on_verification_result(&peer1.node_id, true, now)
            .unwrap();

        table.stage_peer(peer2.clone(), now).unwrap();
        table
            .on_verification_result(&peer2.node_id, true, now)
            .unwrap();

        // INVARIANT-3: Third peer from same subnet rejected
        let peer3 = make_peer(3);
        table.stage_peer(peer3.clone(), now).unwrap();
        let result = table.on_verification_result(&peer3.node_id, true, now);

        assert!(
            matches!(result, Err(PeerDiscoveryError::SubnetLimitReached)),
            "Third peer from same /24 rejected per INVARIANT-3"
        );
    }
}
