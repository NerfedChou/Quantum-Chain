use crate::domain::{BanDetails, NodeId, PeerDiscoveryError, PeerInfo, RoutingTableStats};
use crate::ports::PeerDiscoveryApi;
use crate::service::PeerDiscoveryService;

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

    fn ban_peer(&mut self, node_id: NodeId, details: BanDetails) -> Result<(), PeerDiscoveryError> {
        let now = self.now();
        self.routing_table.ban_peer(node_id, details, now)
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
