use crate::domain::NodeId;
use crate::ipc::security::{SecurityError, SubsystemId};
use crate::ports::inbound::VerificationHandler;

use super::types::{NodeIdentityVerificationResult, SubscriptionError, VerificationOutcome};

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
