use crate::domain::{BanReason, DisconnectReason, NodeId, PeerInfo, WarningType};
use crate::ipc::payloads::{
    BootstrapCompletedPayload, PeerBannedPayload, PeerConnectedPayload, PeerDisconnectedPayload,
    PeerDiscoveryEventPayload, RoutingTableWarningPayload,
};
use crate::ipc::security::SubsystemId;

/// Event builder for creating properly formatted events.
pub struct EventBuilder {
    subsystem_id: u8,
}

impl Default for EventBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBuilder {
    /// Create a new event builder for Peer Discovery.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subsystem_id: SubsystemId::PeerDiscovery.as_u8(),
        }
    }

    /// Get the source subsystem ID.
    #[must_use]
    pub const fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Build a PeerConnected event.
    #[must_use]
    pub fn peer_connected(
        &self,
        peer_info: PeerInfo,
        bucket_index: u8,
    ) -> PeerDiscoveryEventPayload {
        PeerDiscoveryEventPayload::PeerConnected(PeerConnectedPayload {
            peer_info,
            bucket_index,
        })
    }

    /// Build a PeerDisconnected event.
    #[must_use]
    pub fn peer_disconnected(
        &self,
        node_id: NodeId,
        reason: DisconnectReason,
    ) -> PeerDiscoveryEventPayload {
        PeerDiscoveryEventPayload::PeerDisconnected(PeerDisconnectedPayload { node_id, reason })
    }

    /// Build a PeerBanned event.
    #[must_use]
    pub fn peer_banned(
        &self,
        node_id: NodeId,
        reason: BanReason,
        duration_seconds: u64,
    ) -> PeerDiscoveryEventPayload {
        PeerDiscoveryEventPayload::PeerBanned(PeerBannedPayload {
            node_id,
            reason,
            duration_seconds,
        })
    }

    /// Build a BootstrapCompleted event.
    #[must_use]
    pub fn bootstrap_completed(
        &self,
        peer_count: usize,
        duration_ms: u64,
    ) -> PeerDiscoveryEventPayload {
        PeerDiscoveryEventPayload::BootstrapCompleted(BootstrapCompletedPayload {
            peer_count,
            duration_ms,
        })
    }

    /// Build a RoutingTableWarning event.
    #[must_use]
    pub fn routing_table_warning(
        &self,
        warning_type: WarningType,
        details: String,
    ) -> PeerDiscoveryEventPayload {
        PeerDiscoveryEventPayload::RoutingTableWarning(RoutingTableWarningPayload {
            warning_type,
            details,
        })
    }
}
