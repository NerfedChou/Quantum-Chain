//! # Peer Discovery Service
//!
//! High-level service implementing the `PeerDiscoveryApi` port.
//!
//! This service wraps the domain `RoutingTable` and provides a clean API
//! for consumers, hiding the internal complexity of time management and
//! the verification workflow.
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 3.1

use crate::domain::{
    BanReason, KademliaConfig, NodeId, PeerDiscoveryError, PeerInfo, RoutingTable,
    RoutingTableStats, Timestamp,
};
use crate::ports::{PeerDiscoveryApi, TimeSource};

/// Peer Discovery Service implementing the driving port.
///
/// This service provides the primary API for interacting with peer discovery.
/// It wraps a `RoutingTable` and a `TimeSource` to provide time-aware operations.
///
/// # Example
///
/// ```rust,ignore
/// use qc_01_peer_discovery::service::PeerDiscoveryService;
/// use qc_01_peer_discovery::ports::{PeerDiscoveryApi, TimeSource};
///
/// let time_source = SystemTimeSource::new();
/// let config = KademliaConfig::default();
/// let local_id = NodeId::new([0u8; 32]);
/// let mut service = PeerDiscoveryService::new(local_id, config, Box::new(time_source));
///
/// // Use via the trait
/// let stats = service.get_stats();
/// ```
pub struct PeerDiscoveryService {
    /// The underlying routing table (domain layer)
    routing_table: RoutingTable,
    /// Time source for operations requiring timestamps
    time_source: Box<dyn TimeSource>,
}

impl PeerDiscoveryService {
    /// Create a new peer discovery service.
    ///
    /// # Arguments
    ///
    /// * `local_node_id` - Our own node ID
    /// * `config` - Kademlia configuration
    /// * `time_source` - Provider for current time
    pub fn new(
        local_node_id: NodeId,
        config: KademliaConfig,
        time_source: Box<dyn TimeSource>,
    ) -> Self {
        Self {
            routing_table: RoutingTable::new(local_node_id, config),
            time_source,
        }
    }

    /// Get the current timestamp from the time source.
    fn now(&self) -> Timestamp {
        self.time_source.now()
    }

    /// Get the underlying routing table (for advanced operations).
    pub fn routing_table(&self) -> &RoutingTable {
        &self.routing_table
    }

    /// Get mutable access to the routing table.
    pub fn routing_table_mut(&mut self) -> &mut RoutingTable {
        &mut self.routing_table
    }

    /// Handle verification result from Subsystem 10.
    ///
    /// This is called after Subsystem 10 verifies a peer's signature.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The peer that was verified
    /// * `identity_valid` - true if signature verified, false if failed
    ///
    /// # Returns
    ///
    /// If a peer needs to be challenged (bucket full), returns the challenged peer's NodeId.
    /// The caller should send a PING to this peer via `NetworkSocket`.
    pub fn on_verification_result(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        let now = self.now();
        self.routing_table
            .on_verification_result(node_id, identity_valid, now)
    }

    /// Handle challenge response (PING/PONG result).
    ///
    /// # Arguments
    ///
    /// * `challenged_peer` - The peer that was challenged
    /// * `is_alive` - true if peer responded, false if timed out
    pub fn on_challenge_response(
        &mut self,
        challenged_peer: &NodeId,
        is_alive: bool,
    ) -> Result<(), PeerDiscoveryError> {
        let now = self.now();
        self.routing_table
            .on_challenge_response(challenged_peer, is_alive, now)
    }

    /// Run garbage collection to clean expired entries.
    ///
    /// Should be called periodically (e.g., every 60 seconds).
    ///
    /// # Returns
    ///
    /// Number of entries removed.
    pub fn gc(&mut self) -> usize {
        let now = self.now();
        self.routing_table.gc_expired(now)
    }

    /// Check for expired challenges and process them.
    ///
    /// Should be called periodically (e.g., every second).
    ///
    /// Expired challenges are treated as "peer is dead" and the
    /// candidate peer is inserted.
    pub fn check_expired_challenges(&mut self) -> Vec<(usize, PeerInfo, NodeId)> {
        let now = self.now();
        self.routing_table.check_expired_challenges(now)
    }
}

impl PeerDiscoveryApi for PeerDiscoveryService {
    fn find_closest_peers(&self, target_id: NodeId, count: usize) -> Vec<PeerInfo> {
        self.routing_table.find_closest_peers(&target_id, count)
    }

    fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
        let now = self.now();
        self.routing_table.stage_peer(peer, now)
    }

    fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
        self.routing_table.get_random_peers(count)
    }

    fn ban_peer(
        &mut self,
        node_id: NodeId,
        duration_seconds: u64,
        reason: BanReason,
    ) -> Result<(), PeerDiscoveryError> {
        let now = self.now();
        self.routing_table
            .ban_peer(node_id, duration_seconds, reason, now)
    }

    fn is_banned(&self, node_id: NodeId) -> bool {
        let now = self.now();
        self.routing_table.is_banned(&node_id, now)
    }

    fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
        let now = self.now();
        self.routing_table.touch_peer(&node_id, now)
    }

    fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
        self.routing_table.remove_peer(&node_id)
    }

    fn get_stats(&self) -> RoutingTableStats {
        let now = self.now();
        self.routing_table.stats(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IpAddr, SocketAddr};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Mock time source for testing (thread-safe)
    struct MockTimeSource {
        time: AtomicU64,
    }

    impl MockTimeSource {
        fn new(initial: u64) -> Self {
            Self {
                time: AtomicU64::new(initial),
            }
        }

        #[allow(dead_code)]
        fn advance(&self, secs: u64) {
            self.time.fetch_add(secs, Ordering::SeqCst);
        }
    }

    impl TimeSource for MockTimeSource {
        fn now(&self) -> Timestamp {
            Timestamp::new(self.time.load(Ordering::SeqCst))
        }
    }

    fn make_node_id(val: u8) -> NodeId {
        let mut bytes = [0u8; 32];
        bytes[0] = val;
        NodeId::new(bytes)
    }

    fn make_peer(val: u8) -> PeerInfo {
        PeerInfo::new(
            make_node_id(val),
            SocketAddr::new(IpAddr::v4(192, 168, val, 1), 8080),
            Timestamp::new(1000),
        )
    }

    #[test]
    fn test_service_add_peer_stages_for_verification() {
        let local_id = make_node_id(0);
        let config = KademliaConfig::for_testing();
        let time_source = Box::new(MockTimeSource::new(1000));
        let mut service = PeerDiscoveryService::new(local_id, config, time_source);

        let peer = make_peer(1);
        let result = service.add_peer(peer);

        assert!(result.is_ok());
        assert!(result.unwrap()); // Should be staged

        let stats = service.get_stats();
        assert_eq!(stats.pending_verification_count, 1);
        assert_eq!(stats.total_peers, 0); // Not in routing table yet
    }

    #[test]
    fn test_service_verification_promotes_peer() {
        let local_id = make_node_id(0);
        let config = KademliaConfig::for_testing();
        let time_source = Box::new(MockTimeSource::new(1000));
        let mut service = PeerDiscoveryService::new(local_id, config, time_source);

        let peer = make_peer(1);
        let node_id = peer.node_id;

        // Stage peer
        service.add_peer(peer).unwrap();
        assert_eq!(service.get_stats().pending_verification_count, 1);

        // Verify peer
        let result = service.on_verification_result(&node_id, true).unwrap();
        assert!(result.is_none()); // No challenge needed

        let stats = service.get_stats();
        assert_eq!(stats.pending_verification_count, 0);
        assert_eq!(stats.total_peers, 1); // Now in routing table
    }

    #[test]
    fn test_service_failed_verification_drops_peer() {
        let local_id = make_node_id(0);
        let config = KademliaConfig::for_testing();
        let time_source = Box::new(MockTimeSource::new(1000));
        let mut service = PeerDiscoveryService::new(local_id, config, time_source);

        let peer = make_peer(1);
        let node_id = peer.node_id;

        // Stage peer
        service.add_peer(peer).unwrap();

        // Verification fails
        let result = service.on_verification_result(&node_id, false).unwrap();
        assert!(result.is_none());

        let stats = service.get_stats();
        assert_eq!(stats.pending_verification_count, 0);
        assert_eq!(stats.total_peers, 0); // Not added
    }

    #[test]
    fn test_service_implements_api_trait() {
        let local_id = make_node_id(0);
        let config = KademliaConfig::for_testing();
        let time_source = Box::new(MockTimeSource::new(1000));
        let service = PeerDiscoveryService::new(local_id, config, time_source);

        // Test via trait
        fn use_api<T: PeerDiscoveryApi>(api: &T) -> RoutingTableStats {
            api.get_stats()
        }

        let stats = use_api(&service);
        assert_eq!(stats.total_peers, 0);
    }

    #[test]
    fn test_service_ban_and_is_banned() {
        let local_id = make_node_id(0);
        let config = KademliaConfig::for_testing();
        let time_source = Box::new(MockTimeSource::new(1000));
        let mut service = PeerDiscoveryService::new(local_id, config, time_source);

        let peer_id = make_node_id(1);

        assert!(!service.is_banned(peer_id));

        service
            .ban_peer(peer_id, 3600, BanReason::MalformedMessage)
            .unwrap();

        assert!(service.is_banned(peer_id));
    }
}
