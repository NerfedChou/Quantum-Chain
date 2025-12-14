//! # Event Subscriber Adapter
//!
//! Subscribes to events from other subsystems via the shared event bus.
//!
//! ## Events Subscribed (per IPC-MATRIX.md)
//!
//! - From Subsystem 10 (Signature Verification): `NodeIdentityVerificationResult`
//!
//! This allows Peer Discovery to verify node identities at the network edge
//! for DDoS defense.
//!
//! ## EDA Pattern (Architecture.md v2.3)
//!
//! This adapter implements the Event-Driven Architecture pattern:
//! - Receives events from the shared bus
//! - Validates sender authorization per IPC-MATRIX
//! - Routes to the domain service for processing
//! - Emits resulting events via the publisher

use crate::domain::NodeId;
use crate::ipc::security::{SecurityError, SubsystemId};

/// Response from Subsystem 10 for node identity verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeIdentityVerificationResult {
    /// The node ID that was verified.
    pub node_id: [u8; 32],
    /// Whether the identity is valid.
    pub identity_valid: bool,
    /// Timestamp of verification.
    pub verification_timestamp: u64,
}

/// Trait for handling verification results from Subsystem 10.
///
/// This is the EDA contract that the service layer implements.
/// Separating this from `PeerDiscoveryApi` keeps the API clean
/// while enabling proper event-driven integration.
pub trait VerificationHandler: Send + Sync {
    /// Handle verification result from Subsystem 10.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(node_id))` - A peer needs to be challenged (bucket full)
    /// - `Ok(None)` - Verification processed successfully
    /// - `Err(_)` - Error during processing
    fn handle_verification(
        &mut self,
        node_id: &NodeId,
        identity_valid: bool,
    ) -> Result<Option<NodeId>, crate::domain::PeerDiscoveryError>;
}

/// Event subscription port for peer discovery.
///
/// This trait abstracts event subscription to allow testing without
/// the actual shared-bus infrastructure.
pub trait PeerDiscoveryEventSubscriber: Send + Sync {
    /// Subscribe to node identity verification results from Subsystem 10.
    ///
    /// This is called when Subsystem 10 finishes verifying a peer's signature.
    fn on_node_identity_result(
        &mut self,
        sender_id: u8,
        result: NodeIdentityVerificationResult,
    ) -> Result<VerificationOutcome, SubscriptionError>;
}

/// Outcome of processing a verification result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// Peer was promoted to routing table.
    PeerPromoted {
        /// The promoted peer's node ID.
        node_id: [u8; 32],
    },
    /// Peer was rejected (invalid signature).
    PeerRejected {
        /// The rejected peer's node ID.
        node_id: [u8; 32],
    },
    /// Bucket is full, need to challenge existing peer.
    ChallengeRequired {
        /// The new peer waiting to be added.
        new_peer: [u8; 32],
        /// The existing peer being challenged.
        challenged_peer: [u8; 32],
    },
    /// Node was not in pending verification (already processed or unknown).
    NotFound {
        /// The node ID that was not found.
        node_id: [u8; 32],
    },
}

/// Errors that can occur during event subscription processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    /// Message failed security validation.
    SecurityViolation(SecurityError),
    /// The event was for an unknown node.
    UnknownNode {
        /// The unknown node's ID.
        node_id: [u8; 32],
    },
    /// Processing error.
    ProcessingError(String),
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecurityViolation(e) => write!(f, "security violation: {e}"),
            Self::UnknownNode { node_id } => {
                write!(f, "unknown node: {:?}", &node_id[..4])
            }
            Self::ProcessingError(msg) => write!(f, "processing error: {msg}"),
        }
    }
}

impl std::error::Error for SubscriptionError {}

impl From<SecurityError> for SubscriptionError {
    fn from(e: SecurityError) -> Self {
        Self::SecurityViolation(e)
    }
}

/// Validates that a sender is authorized to send identity verification results.
///
/// Per IPC-MATRIX.md, only Subsystem 10 (Signature Verification) can send these.
pub fn validate_identity_result_sender(sender_id: u8) -> Result<(), SecurityError> {
    if sender_id != SubsystemId::SignatureVerification.as_u8() {
        return Err(SecurityError::UnauthorizedSender {
            sender_id,
            allowed_senders: &[10], // Only Subsystem 10
        });
    }
    Ok(())
}

/// Event handler that connects to the peer discovery service.
///
/// This struct handles incoming events and routes them to the service.
/// It implements the EDA pattern by:
/// 1. Receiving events from the shared bus
/// 2. Validating authorization per IPC-MATRIX
/// 3. Routing to domain service for processing
/// 4. Returning outcomes that can trigger further events
pub struct EventHandler<S> {
    /// The peer discovery service to route events to.
    service: S,
}

impl<S> EventHandler<S> {
    /// Create a new event handler.
    pub fn new(service: S) -> Self {
        Self { service }
    }

    /// Get a reference to the inner service.
    pub fn service(&self) -> &S {
        &self.service
    }

    /// Get a mutable reference to the inner service.
    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    /// Consume the handler and return the inner service.
    pub fn into_service(self) -> S {
        self.service
    }
}

impl<S: VerificationHandler> PeerDiscoveryEventSubscriber for EventHandler<S> {
    fn on_node_identity_result(
        &mut self,
        sender_id: u8,
        result: NodeIdentityVerificationResult,
    ) -> Result<VerificationOutcome, SubscriptionError> {
        // IPC-MATRIX.md: Validate sender is Subsystem 10 (Signature Verification)
        validate_identity_result_sender(sender_id)?;

        // Convert raw bytes to domain NodeId type
        let node_id = NodeId::new(result.node_id);

        // SPEC-01 Section 4.3: Identity result triggers routing table state transition
        // This is the core EDA action - process the event and return the outcome
        match self
            .service
            .handle_verification(&node_id, result.identity_valid)
        {
            Ok(Some(challenged_peer)) => {
                // Bucket was full, need to challenge existing peer
                // This outcome can trigger a PING event via the publisher
                Ok(VerificationOutcome::ChallengeRequired {
                    new_peer: result.node_id,
                    challenged_peer: *challenged_peer.as_bytes(),
                })
            }
            Ok(None) => {
                // Successfully processed
                if result.identity_valid {
                    Ok(VerificationOutcome::PeerPromoted {
                        node_id: result.node_id,
                    })
                } else {
                    Ok(VerificationOutcome::PeerRejected {
                        node_id: result.node_id,
                    })
                }
            }
            Err(crate::domain::PeerDiscoveryError::PeerNotFound) => {
                // Peer was not in pending verification (already processed or unknown)
                Ok(VerificationOutcome::NotFound {
                    node_id: result.node_id,
                })
            }
            Err(e) => Err(SubscriptionError::ProcessingError(e.to_string())),
        }
    }
}

/// Filter for subscription to specific event types.
#[derive(Debug, Clone, Default)]
pub struct SubscriptionFilter {
    /// Subsystems to accept events from.
    pub allowed_senders: Vec<u8>,
    /// Event types to accept (empty = all).
    pub event_types: Vec<String>,
}

impl SubscriptionFilter {
    /// Create a filter that accepts only Signature Verification results.
    #[must_use]
    pub fn signature_verification_only() -> Self {
        Self {
            allowed_senders: vec![SubsystemId::SignatureVerification.as_u8()],
            event_types: vec!["NodeIdentityVerificationResult".to_string()],
        }
    }

    /// Check if a sender is allowed by this filter.
    #[must_use]
    pub fn is_sender_allowed(&self, sender_id: u8) -> bool {
        self.allowed_senders.is_empty() || self.allowed_senders.contains(&sender_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_event_handler_promotes_peer_on_valid_verification() {
        let mut service = MockVerificationService::new();
        service.stage_peer([1u8; 32]);
        let mut handler = EventHandler::new(service);

        let result = handler.on_node_identity_result(
            10, // Correct sender ID
            NodeIdentityVerificationResult {
                node_id: [1u8; 32],
                identity_valid: true,
                verification_timestamp: 1000,
            },
        );

        // Should return PeerPromoted
        assert!(matches!(
            result,
            Ok(VerificationOutcome::PeerPromoted { node_id }) if node_id == [1u8; 32]
        ));

        // Verify peer was actually added to routing table
        assert_eq!(handler.service().peer_count(), 1);
    }

    #[test]
    fn test_event_handler_rejects_peer_on_invalid_verification() {
        let mut service = MockVerificationService::new();
        service.stage_peer([1u8; 32]);
        let mut handler = EventHandler::new(service);

        let result = handler.on_node_identity_result(
            10,
            NodeIdentityVerificationResult {
                node_id: [1u8; 32],
                identity_valid: false, // Invalid signature
                verification_timestamp: 1000,
            },
        );

        // Should return PeerRejected
        assert!(matches!(
            result,
            Ok(VerificationOutcome::PeerRejected { node_id }) if node_id == [1u8; 32]
        ));

        // Verify peer was NOT added to routing table
        assert_eq!(handler.service().peer_count(), 0);
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
}
