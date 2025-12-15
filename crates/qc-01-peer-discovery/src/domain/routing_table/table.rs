//! Main RoutingTable implementation.
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 2.2

use std::collections::HashMap;

use crate::domain::{
    calculate_bucket_index, is_same_subnet, Distance, KademliaConfig, NodeId, PeerDiscoveryError,
    PeerInfo, SubnetMask, Timestamp,
};

use super::banned::BannedPeers;
use super::bucket::KBucket;
use super::config::NUM_BUCKETS;
use super::security::{BanDetails, PendingInsertion, PendingPeer, RoutingTableStats};

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
    pending_verification: HashMap<NodeId, PendingPeer>,
    /// Configuration including max_pending_peers limit
    config: KademliaConfig,
    /// Subnet mask for IP diversity checks
    subnet_mask: SubnetMask,
}

impl RoutingTable {
    /// Create a new routing table
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
            .flat_map(|b| b.peers().iter())
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
    pub fn stage_peer(
        &mut self,
        peer: PeerInfo,
        now: Timestamp,
    ) -> Result<bool, PeerDiscoveryError> {
        // INVARIANT-9: Check staging capacity FIRST
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
    /// # INVARIANT-10: Eviction-on-Failure for full buckets
    pub fn on_verification_result(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
        now: Timestamp,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        let pending = self.pending_verification.remove(node_id);

        if !identity_valid {
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
            .peers()
            .iter()
            .filter(|p| is_same_subnet(&p.socket_addr.ip, &peer.socket_addr.ip, &self.subnet_mask))
            .count();

        if peers_in_subnet >= self.config.max_peers_per_subnet {
            return Err(PeerDiscoveryError::SubnetLimitReached);
        }

        // INVARIANT-1: Check bucket capacity
        if !bucket.is_full(self.config.k) {
            bucket.add_peer(peer, now);
            return Ok(None);
        }

        // Bucket is full - need to challenge oldest peer
        if bucket.has_pending_challenge() {
            return Err(PeerDiscoveryError::ChallengeInProgress);
        }

        let oldest = bucket.oldest_peer().ok_or(PeerDiscoveryError::BucketFull)?;
        let challenged_peer = oldest.node_id;

        bucket.pending_insertion = Some(PendingInsertion {
            candidate: peer,
            challenged_peer,
            challenge_sent_at: now,
            challenge_deadline: now.add_secs(self.config.eviction_challenge_timeout_secs),
        });

        Ok(Some(challenged_peer))
    }

    /// Handle challenge response (PING/PONG result)
    pub fn on_challenge_response(
        &mut self,
        challenged_peer: &NodeId,
        is_alive: bool,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        let bucket_idx = calculate_bucket_index(&self.local_node_id, challenged_peer);
        let bucket = self
            .buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)?;

        let pending = bucket
            .pending_insertion
            .take()
            .ok_or(PeerDiscoveryError::PeerNotFound)?;

        if &pending.challenged_peer != challenged_peer {
            bucket.pending_insertion = Some(pending);
            return Err(PeerDiscoveryError::PeerNotFound);
        }

        if is_alive {
            bucket.move_to_front(challenged_peer, now);
        } else {
            bucket.remove_peer(challenged_peer);
            bucket.add_peer(pending.candidate, now);
        }

        Ok(())
    }

    /// Check for expired eviction challenges
    pub fn check_expired_challenges(&mut self, now: Timestamp) -> Vec<(usize, PeerInfo, NodeId)> {
        let mut expired = Vec::new();

        for (idx, bucket) in self.buckets.iter_mut().enumerate() {
            let Some(ref pending) = bucket.pending_insertion else {
                continue;
            };
            if now < pending.challenge_deadline {
                continue;
            }
            let Some(pending) = bucket.pending_insertion.take() else {
                continue;
            };
            expired.push((idx, pending.candidate, pending.challenged_peer));
        }

        for (idx, candidate, challenged) in &expired {
            if let Some(bucket) = self.buckets.get_mut(*idx) {
                bucket.remove_peer(challenged);
                bucket.add_peer(candidate.clone(), now);
            }
        }

        expired
    }

    /// Garbage collect expired entries
    pub fn gc_expired(&mut self, now: Timestamp) -> usize {
        let mut removed = 0;

        let before = self.pending_verification.len();
        self.pending_verification
            .retain(|_, p| p.verification_deadline > now);
        removed += before - self.pending_verification.len();

        removed += self.banned_peers.gc_expired(now);

        removed
    }

    /// Ban a peer
    pub fn ban_peer(
        &mut self,
        node_id: NodeId,
        details: BanDetails,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        let bucket_idx = calculate_bucket_index(&self.local_node_id, &node_id);
        if let Some(bucket) = self.buckets.get_mut(bucket_idx) {
            bucket.remove_peer(&node_id);
        }

        self.pending_verification.remove(&node_id);

        let until = now.add_secs(details.duration_secs);
        self.banned_peers.ban(node_id, until, details.reason);

        Ok(())
    }

    /// Check if a peer is banned
    pub fn is_banned(&self, node_id: &NodeId, now: Timestamp) -> bool {
        self.banned_peers.is_banned(node_id, now)
    }

    /// Helper to get mutable bucket for a node ID
    fn get_bucket_mut_for_node(
        &mut self,
        node_id: &NodeId,
    ) -> Result<&mut KBucket, PeerDiscoveryError> {
        let bucket_idx = calculate_bucket_index(&self.local_node_id, node_id);
        self.buckets
            .get_mut(bucket_idx)
            .ok_or(PeerDiscoveryError::InvalidNodeId)
    }

    /// Touch a peer (update last_seen)
    pub fn touch_peer(
        &mut self,
        node_id: &NodeId,
        now: Timestamp,
    ) -> Result<(), PeerDiscoveryError> {
        let bucket = self.get_bucket_mut_for_node(node_id)?;

        if bucket.touch_peer(node_id, now) {
            Ok(())
        } else {
            Err(PeerDiscoveryError::PeerNotFound)
        }
    }

    /// Remove a peer from the routing table
    pub fn remove_peer(&mut self, node_id: &NodeId) -> Result<(), PeerDiscoveryError> {
        let bucket = self.get_bucket_mut_for_node(node_id)?;

        bucket
            .remove_peer(node_id)
            .map(|_| ())
            .ok_or(PeerDiscoveryError::PeerNotFound)
    }

    /// Find the k closest peers to a target
    pub fn find_closest_peers(&self, target: &NodeId, count: usize) -> Vec<PeerInfo> {
        let mut all_peers: Vec<(Distance, &PeerInfo)> = self
            .buckets
            .iter()
            .flat_map(|b| b.peers().iter())
            .map(|p| (crate::domain::xor_distance(&p.node_id, target), p))
            .collect();

        all_peers.sort_by(|a, b| b.0.cmp(&a.0));

        all_peers
            .into_iter()
            .take(count)
            .map(|(_, p)| p.clone())
            .collect()
    }

    /// Get random peers for gossip protocols
    pub fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
        self.buckets
            .iter()
            .flat_map(|b| b.peers().iter().cloned())
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
