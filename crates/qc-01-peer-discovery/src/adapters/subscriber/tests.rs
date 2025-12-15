use super::*;
use crate::domain::NodeId;
use crate::ipc::security::SecurityError;
use crate::ports::inbound::VerificationHandler;

#[test]
fn test_validate_identity_result_sender_authorized() {
    // Subsystem 10 is authorized
    assert!(validate_identity_result_sender(10).is_ok());
}

#[test]
fn test_validate_identity_result_sender_unauthorized() {
    // Other subsystems are not authorized
    assert!(matches!(
        validate_identity_result_sender(1),
        Err(SecurityError::UnauthorizedSender { .. })
    ));
    assert!(matches!(
        validate_identity_result_sender(5),
        Err(SecurityError::UnauthorizedSender { .. })
    ));
    assert!(matches!(
        validate_identity_result_sender(8),
        Err(SecurityError::UnauthorizedSender { .. })
    ));
}

#[test]
fn test_node_identity_verification_result() {
    let result = NodeIdentityVerificationResult {
        node_id: [1u8; 32],
        identity_valid: true,
        verification_timestamp: 1000,
    };
    assert!(result.identity_valid);
    assert_eq!(result.verification_timestamp, 1000);
}

#[test]
fn test_subscription_filter_signature_verification() {
    let filter = SubscriptionFilter::signature_verification_only();
    assert!(filter.is_sender_allowed(10)); // Signature Verification
    assert!(!filter.is_sender_allowed(1)); // Peer Discovery
    assert!(!filter.is_sender_allowed(8)); // Consensus
}

#[test]
fn test_subscription_filter_empty_allows_all() {
    let filter = SubscriptionFilter::default();
    assert!(filter.is_sender_allowed(1));
    assert!(filter.is_sender_allowed(10));
    assert!(filter.is_sender_allowed(255));
}

#[test]
fn test_subscription_error_display() {
    let err = SubscriptionError::UnknownNode { node_id: [1u8; 32] };
    let msg = err.to_string();
    assert!(msg.contains("unknown node"));

    let err = SubscriptionError::ProcessingError("test error".to_string());
    let msg = err.to_string();
    assert!(msg.contains("test error"));
}

#[test]
fn test_subscription_error_from_security_error() {
    let security_err = SecurityError::UnauthorizedSender {
        sender_id: 5,
        allowed_senders: &[10],
    };
    let sub_err: SubscriptionError = security_err.into();
    assert!(matches!(sub_err, SubscriptionError::SecurityViolation(_)));
}

// ========================================================================
// EDA Integration Tests - Event Handler with VerificationHandler
// ========================================================================

use crate::domain::{
    IpAddr, KademliaConfig, PeerDiscoveryError, PeerInfo, RoutingTable, SocketAddr, Timestamp,
};

/// Mock service that implements VerificationHandler for testing
struct MockVerificationService {
    routing_table: RoutingTable,
    current_time: Timestamp,
}

impl MockVerificationService {
    fn new() -> Self {
        let local_id = NodeId::new([0u8; 32]);
        Self {
            routing_table: RoutingTable::new(local_id, KademliaConfig::for_testing()),
            current_time: Timestamp::new(1000),
        }
    }

    fn stage_peer(&mut self, node_id: [u8; 32]) {
        let peer = PeerInfo::new(
            NodeId::new(node_id),
            SocketAddr::new(IpAddr::v4(192, 168, 1, 1), 8080),
            self.current_time,
        );
        self.routing_table.stage_peer(peer, self.current_time).ok();
    }

    fn peer_count(&self) -> usize {
        self.routing_table.stats(self.current_time).total_peers
    }
}

impl VerificationHandler for MockVerificationService {
    fn handle_verification(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
    ) -> Result<Option<NodeId>, PeerDiscoveryError> {
        self.routing_table
            .on_verification_result(node_id, identity_valid, self.current_time)
    }
}

#[test]
fn test_event_handler_rejects_unauthorized_sender() {
    let service = MockVerificationService::new();
    let mut handler = EventHandler::new(service);

    let result = handler.on_node_identity_result(
        5, // Wrong sender ID - should be 10
        NodeIdentityVerificationResult {
            node_id: [1u8; 32],
            identity_valid: true,
            verification_timestamp: 1000,
        },
    );

    assert!(matches!(
        result,
        Err(SubscriptionError::SecurityViolation(_))
    ));
}

fn verify_scenario(
    node_id_byte: u8,
    valid: bool,
) -> (VerificationOutcome, MockVerificationService) {
    let mut service = MockVerificationService::new();
    service.stage_peer([node_id_byte; 32]);
    let mut handler = EventHandler::new(service);

    let result = handler
        .on_node_identity_result(
            10, // Correct sender ID
            NodeIdentityVerificationResult {
                node_id: [node_id_byte; 32],
                identity_valid: valid,
                verification_timestamp: 1000,
            },
        )
        .expect("Handler execution failed"); // Unwrap here for test helper simplicity

    (result, handler.into_service())
}

#[test]
fn test_event_handler_promotes_peer_on_valid_verification() {
    let (result, service) = verify_scenario(1, true);

    // Should return PeerPromoted
    assert!(matches!(
        result,
        VerificationOutcome::PeerPromoted { node_id } if node_id == [1u8; 32]
    ));

    // Verify peer was actually added to routing table
    assert_eq!(service.peer_count(), 1);
}

#[test]
fn test_event_handler_rejects_peer_on_invalid_verification() {
    let (result, service) = verify_scenario(1, false);

    // Should return PeerRejected
    assert!(matches!(
        result,
        VerificationOutcome::PeerRejected { node_id } if node_id == [1u8; 32]
    ));

    // Verify peer was NOT added to routing table
    assert_eq!(service.peer_count(), 0);
}

#[test]
fn test_event_handler_returns_not_found_for_unknown_peer() {
    let service = MockVerificationService::new();
    let mut handler = EventHandler::new(service);

    // Try to verify a peer that was never staged
    let result = handler.on_node_identity_result(
        10,
        NodeIdentityVerificationResult {
            node_id: [99u8; 32], // Unknown peer
            identity_valid: true,
            verification_timestamp: 1000,
        },
    );

    // Should return NotFound
    assert!(matches!(
        result,
        Ok(VerificationOutcome::NotFound { node_id }) if node_id == [99u8; 32]
    ));
}

#[test]
fn test_event_handler_into_service_consumes_handler() {
    let service = MockVerificationService::new();
    let handler = EventHandler::new(service);

    let recovered_service = handler.into_service();
    assert_eq!(recovered_service.peer_count(), 0);
}
