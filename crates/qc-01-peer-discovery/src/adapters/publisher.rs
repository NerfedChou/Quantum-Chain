//! # Event Publisher Adapter
//!
//! Publishes peer discovery events to the shared event bus.
//!
//! ## Events Published
//!
//! Per SPEC-01 Section 4.1:
//! - `PeerConnected` - When a peer is successfully added to routing table
//! - `PeerDisconnected` - When a peer is removed
//! - `PeerBanned` - When a peer is banned
//! - `BootstrapCompleted` - When bootstrap process finishes
//! - `RoutingTableWarning` - When health issues are detected

use crate::domain::{BanReason, DisconnectReason, NodeId, PeerInfo, WarningType};
use crate::ipc::payloads::{
    BootstrapCompletedPayload, PeerBannedPayload, PeerConnectedPayload, PeerDisconnectedPayload,
    PeerDiscoveryEventPayload, PeerListResponsePayload, RoutingTableWarningPayload,
};
use crate::ipc::security::SubsystemId;

/// Event publishing port for peer discovery.
///
/// This trait abstracts the event bus to allow testing without
/// the actual shared-bus infrastructure.
pub trait PeerDiscoveryEventPublisher: Send + Sync {
    /// Publish an event to the bus.
    ///
    /// # Arguments
    ///
    /// * `event` - The event payload to publish
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error message.
    fn publish(&self, event: PeerDiscoveryEventPayload) -> Result<(), String>;

    /// Publish a response to a specific topic (for request/response flows).
    ///
    /// # Arguments
    ///
    /// * `topic` - The reply_to topic from the original request
    /// * `correlation_id` - The correlation ID from the original request
    /// * `response` - The response payload
    fn publish_response(
        &self,
        topic: &str,
        correlation_id: [u8; 16],
        response: PeerListResponsePayload,
    ) -> Result<(), String>;
}

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

/// No-op publisher for testing.
#[derive(Debug, Default)]
pub struct NoOpEventPublisher {
    /// Count of published events (for testing verification).
    pub event_count: std::sync::atomic::AtomicUsize,
}

impl NoOpEventPublisher {
    /// Create a new no-op publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get the count of published events.
    #[must_use]
    pub fn get_event_count(&self) -> usize {
        self.event_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl PeerDiscoveryEventPublisher for NoOpEventPublisher {
    fn publish(&self, _event: PeerDiscoveryEventPayload) -> Result<(), String> {
        self.event_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn publish_response(
        &self,
        _topic: &str,
        _correlation_id: [u8; 16],
        _response: PeerListResponsePayload,
    ) -> Result<(), String> {
        self.event_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

/// In-memory publisher for testing that stores events.
#[derive(Debug, Default)]
pub struct InMemoryEventPublisher {
    events: std::sync::Mutex<Vec<PeerDiscoveryEventPayload>>,
}

impl InMemoryEventPublisher {
    /// Create a new in-memory publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all published events.
    #[must_use]
    pub fn get_events(&self) -> Vec<PeerDiscoveryEventPayload> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all stored events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl PeerDiscoveryEventPublisher for InMemoryEventPublisher {
    fn publish(&self, event: PeerDiscoveryEventPayload) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    fn publish_response(
        &self,
        _topic: &str,
        _correlation_id: [u8; 16],
        response: PeerListResponsePayload,
    ) -> Result<(), String> {
        self.events
            .lock()
            .unwrap()
            .push(PeerDiscoveryEventPayload::PeerListResponse(response));
        Ok(())
    }
}

// =============================================================================
// VERIFICATION REQUEST PUBLISHER (Outbound to Subsystem 10)
// =============================================================================


use crate::ipc::VerifyNodeIdentityRequest;

/// Publisher for sending verification requests to Subsystem 10.
///
/// This is the EDA outbound port for the DDoS defense flow.
/// When a new peer connects via `BootstrapRequest`, we stage them
/// and send a verification request to Subsystem 10.
///
/// ## Flow
///
/// ```text
/// BootstrapRequest → stage peer → publish_verification_request → Subsystem 10
/// ```
pub trait VerificationRequestPublisher: Send + Sync {
    /// Send a verification request to Subsystem 10.
    ///
    /// # Arguments
    ///
    /// * `request` - The verification request payload
    /// * `correlation_id` - ID to correlate with the eventual response
    ///
    /// # Returns
    ///
    /// Ok(()) if published successfully
    fn publish_verification_request(
        &self,
        request: VerifyNodeIdentityRequest,
        correlation_id: [u8; 16],
    ) -> Result<(), String>;
}

/// No-op verification request publisher for testing.
#[derive(Debug, Default)]
pub struct NoOpVerificationPublisher {
    /// Count of requests published.
    pub request_count: std::sync::atomic::AtomicUsize,
}

impl NoOpVerificationPublisher {
    /// Create a new no-op publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            request_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get the count of published requests.
    #[must_use]
    pub fn get_request_count(&self) -> usize {
        self.request_count
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl VerificationRequestPublisher for NoOpVerificationPublisher {
    fn publish_verification_request(
        &self,
        _request: VerifyNodeIdentityRequest,
        _correlation_id: [u8; 16],
    ) -> Result<(), String> {
        self.request_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

/// In-memory verification publisher for testing.
#[derive(Debug, Default)]
pub struct InMemoryVerificationPublisher {
    requests: std::sync::Mutex<Vec<(VerifyNodeIdentityRequest, [u8; 16])>>,
}

impl InMemoryVerificationPublisher {
    /// Create a new in-memory publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all published requests.
    #[must_use]
    pub fn get_requests(&self) -> Vec<(VerifyNodeIdentityRequest, [u8; 16])> {
        self.requests.lock().unwrap().clone()
    }

    /// Clear all stored requests.
    pub fn clear(&self) {
        self.requests.lock().unwrap().clear();
    }
}

impl VerificationRequestPublisher for InMemoryVerificationPublisher {
    fn publish_verification_request(
        &self,
        request: VerifyNodeIdentityRequest,
        correlation_id: [u8; 16],
    ) -> Result<(), String> {
        self.requests
            .lock()
            .unwrap()
            .push((request, correlation_id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IpAddr, SocketAddr, Timestamp};

    fn make_peer_info(id_byte: u8) -> PeerInfo {
        let mut id_bytes = [0u8; 32];
        id_bytes[0] = id_byte;
        PeerInfo::new(
            NodeId::new(id_bytes),
            SocketAddr::new(IpAddr::v4(192, 168, 1, id_byte), 8080),
            Timestamp::new(1000),
        )
    }

    #[test]
    fn test_event_builder_peer_connected() {
        let builder = EventBuilder::new();
        let peer = make_peer_info(1);
        let event = builder.peer_connected(peer.clone(), 5);

        match event {
            PeerDiscoveryEventPayload::PeerConnected(payload) => {
                assert_eq!(payload.peer_info.node_id, peer.node_id);
                assert_eq!(payload.bucket_index, 5);
            }
            _ => panic!("Expected PeerConnected event"),
        }
    }

    #[test]
    fn test_event_builder_peer_disconnected() {
        let builder = EventBuilder::new();
        let node_id = NodeId::new([1u8; 32]);
        let event = builder.peer_disconnected(node_id, DisconnectReason::Timeout);

        match event {
            PeerDiscoveryEventPayload::PeerDisconnected(payload) => {
                assert_eq!(payload.node_id, node_id);
                assert_eq!(payload.reason, DisconnectReason::Timeout);
            }
            _ => panic!("Expected PeerDisconnected event"),
        }
    }

    #[test]
    fn test_event_builder_peer_banned() {
        let builder = EventBuilder::new();
        let node_id = NodeId::new([1u8; 32]);
        let event = builder.peer_banned(node_id, BanReason::MalformedMessage, 3600);

        match event {
            PeerDiscoveryEventPayload::PeerBanned(payload) => {
                assert_eq!(payload.node_id, node_id);
                assert_eq!(payload.reason, BanReason::MalformedMessage);
                assert_eq!(payload.duration_seconds, 3600);
            }
            _ => panic!("Expected PeerBanned event"),
        }
    }

    #[test]
    fn test_event_builder_bootstrap_completed() {
        let builder = EventBuilder::new();
        let event = builder.bootstrap_completed(50, 5000);

        match event {
            PeerDiscoveryEventPayload::BootstrapCompleted(payload) => {
                assert_eq!(payload.peer_count, 50);
                assert_eq!(payload.duration_ms, 5000);
            }
            _ => panic!("Expected BootstrapCompleted event"),
        }
    }

    #[test]
    fn test_event_builder_routing_table_warning() {
        let builder = EventBuilder::new();
        let event = builder.routing_table_warning(
            WarningType::TooFewPeers,
            "Only 5 peers available".to_string(),
        );

        match event {
            PeerDiscoveryEventPayload::RoutingTableWarning(payload) => {
                assert_eq!(payload.warning_type, WarningType::TooFewPeers);
                assert_eq!(payload.details, "Only 5 peers available");
            }
            _ => panic!("Expected RoutingTableWarning event"),
        }
    }

    #[test]
    fn test_noop_publisher() {
        let publisher = NoOpEventPublisher::new();
        let builder = EventBuilder::new();

        let event = builder.bootstrap_completed(10, 1000);
        assert!(publisher.publish(event).is_ok());
        assert_eq!(publisher.get_event_count(), 1);

        let event2 = builder.peer_disconnected(NodeId::new([1u8; 32]), DisconnectReason::Timeout);
        assert!(publisher.publish(event2).is_ok());
        assert_eq!(publisher.get_event_count(), 2);
    }

    #[test]
    fn test_in_memory_publisher() {
        let publisher = InMemoryEventPublisher::new();
        let builder = EventBuilder::new();

        let event = builder.bootstrap_completed(10, 1000);
        assert!(publisher.publish(event).is_ok());

        let events = publisher.get_events();
        assert_eq!(events.len(), 1);

        match &events[0] {
            PeerDiscoveryEventPayload::BootstrapCompleted(payload) => {
                assert_eq!(payload.peer_count, 10);
            }
            _ => panic!("Expected BootstrapCompleted"),
        }

        publisher.clear();
        assert_eq!(publisher.get_events().len(), 0);
    }
}
