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
    ) -> Result<(), SubscriptionError>;
}

/// Errors that can occur during event subscription processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionError {
    /// Message failed security validation.
    SecurityViolation(SecurityError),
    /// The event was for an unknown node.
    UnknownNode { node_id: [u8; 32] },
    /// Processing error.
    ProcessingError(String),
}

impl std::fmt::Display for SubscriptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecurityViolation(e) => write!(f, "security violation: {}", e),
            Self::UnknownNode { node_id } => {
                write!(f, "unknown node: {:?}", &node_id[..4])
            }
            Self::ProcessingError(msg) => write!(f, "processing error: {}", msg),
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
}

impl<S: crate::ports::PeerDiscoveryApi + Send + Sync> PeerDiscoveryEventSubscriber for EventHandler<S> {
    fn on_node_identity_result(
        &mut self,
        sender_id: u8,
        result: NodeIdentityVerificationResult,
    ) -> Result<(), SubscriptionError> {
        // Validate sender
        validate_identity_result_sender(sender_id)?;

        // Convert to domain type
        let node_id = crate::domain::NodeId::new(result.node_id);

        // Note: We would need to wire this to the service properly
        // For now, this is a structural implementation showing the pattern
        // The actual implementation would use a timestamp from the result
        
        // This would typically call something like:
        // self.service.on_verification_result(&node_id, result.identity_valid)
        
        // For now, we just validate the pattern is correct
        let _ = (node_id, result.identity_valid);
        
        Ok(())
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
}
