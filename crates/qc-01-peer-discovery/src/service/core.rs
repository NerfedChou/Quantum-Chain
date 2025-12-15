use crate::domain::{KademliaConfig, NodeId, RoutingTable, Timestamp};
use crate::ports::TimeSource;

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
    pub(crate) routing_table: RoutingTable,
    /// Time source for operations requiring timestamps
    pub(crate) time_source: Box<dyn TimeSource>,
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
    pub(crate) fn now(&self) -> Timestamp {
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
}
