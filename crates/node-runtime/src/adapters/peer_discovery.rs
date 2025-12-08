use std::sync::Arc;
use parking_lot::RwLock;
use tokio::spawn;
use tracing::{error, info};

use qc_01_peer_discovery::{
    adapters::VerificationRequestPublisher,
    domain::{BanReason, NodeId, PeerDiscoveryError, PeerInfo, RoutingTableStats},
    ipc::VerifyNodeIdentityRequest,
    ports::PeerDiscoveryApi,
    service::PeerDiscoveryService,
};
use shared_bus::{EventPublisher, InMemoryEventBus, events::BlockchainEvent};
use shared_types::ipc::VerifyNodeIdentityPayload;

/// Wrapper around Shared PeerDiscoveryService to implement PeerDiscoveryApi.
/// Allows usage in handlers (like BootstrapHandler) that require ownership or mutable reference,
/// while maintaining shared state via Arc<RwLock>.
pub struct SharedPeerDiscovery {
    pub inner: Arc<RwLock<PeerDiscoveryService>>,
}

impl PeerDiscoveryApi for SharedPeerDiscovery {
    fn find_closest_peers(&self, target_id: NodeId, count: usize) -> Vec<PeerInfo> {
        self.inner.read().find_closest_peers(target_id, count)
    }

    fn add_peer(&mut self, peer: PeerInfo) -> Result<bool, PeerDiscoveryError> {
        self.inner.write().add_peer(peer)
    }

    fn get_random_peers(&self, count: usize) -> Vec<PeerInfo> {
        self.inner.read().get_random_peers(count)
    }

    fn ban_peer(
        &mut self,
        node_id: NodeId,
        duration_seconds: u64,
        reason: BanReason,
    ) -> Result<(), PeerDiscoveryError> {
        self.inner
            .write()
            .ban_peer(node_id, duration_seconds, reason)
    }

    fn is_banned(&self, node_id: NodeId) -> bool {
        self.inner.read().is_banned(node_id)
    }

    fn touch_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
        self.inner.write().touch_peer(node_id)
    }

    fn remove_peer(&mut self, node_id: NodeId) -> Result<(), PeerDiscoveryError> {
        self.inner.write().remove_peer(node_id)
    }

    fn get_stats(&self) -> RoutingTableStats {
        self.inner.read().get_stats()
    }
}

/// Runtime implementation of the Verification Publisher.
///
/// Connects the standalone `qc-01` subsystem to the system's `shared-bus`.
/// Publishes `VerifyNodeIdentity` events to be consumed by Subsystem 10.
pub struct RuntimeVerificationPublisher {
    event_bus: Arc<InMemoryEventBus>,
}

impl RuntimeVerificationPublisher {
    pub fn new(event_bus: Arc<InMemoryEventBus>) -> Self {
        Self { event_bus }
    }
}

impl VerificationRequestPublisher for RuntimeVerificationPublisher {
    fn publish_verification_request(
        &self,
        request: VerifyNodeIdentityRequest,
        correlation_id: [u8; 16],
    ) -> Result<(), String> {
        // Convert [u8; 16] correlation_id to hex string
        let correlation_id_str = hex::encode(correlation_id);

        let payload = VerifyNodeIdentityPayload {
            node_id: shared_types::entities::NodeId(request.node_id),
            // Use direct conversion to Vec, bypassing Serde limitation & truncation
            public_key: request.claimed_pubkey.to_vec(),
            signature: request.signature,
        };

        let event = BlockchainEvent::VerifyNodeIdentity {
            correlation_id: correlation_id_str,
            payload,
        };

        let event_bus = self.event_bus.clone();
        
        // Spawn async task because publish is async but trait is sync
        spawn(async move {
            event_bus.publish(event).await;
        });

        info!("Published verification request for node {:?}", request.node_id);
        Ok(())
    }
}
