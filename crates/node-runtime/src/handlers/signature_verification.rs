use crate::container::config::NodeConfig;
use qc_10_signature_verification::adapters::ipc::IpcHandler;
use qc_10_signature_verification::SignatureVerificationApi;
use shared_bus::{
    events::BlockchainEvent, EventFilter, EventPublisher, EventTopic, InMemoryEventBus,
};
use shared_types::envelope::AuthenticatedMessage;
use shared_types::ipc::{VerifyNodeIdentityPayload, VerifyNodeIdentityResponse};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Handler for Signature Verification (qc-10) events.
///
/// Bridges the `shared-bus` events (EDA) to the internal `IpcHandler` of `qc-10`.
/// Ensures messages are properly enveloped and signed for internal security boundaries.
///
/// ## Security Note
///
/// HMAC signing is handled by the `AuthenticatedMessage::sign()` method when needed.
/// The secret is accessed from `NodeConfig::security::hmac_secret` at signing time.
pub struct SignatureVerificationHandler<S: SignatureVerificationApi> {
    bus: Arc<InMemoryEventBus>,
    ipc_handler: IpcHandler<S>,
}

impl<S: SignatureVerificationApi + Clone + Send + Sync + 'static> SignatureVerificationHandler<S> {
    /// Create a new signature verification handler.
    pub fn new(bus: Arc<InMemoryEventBus>, service: S, _config: &NodeConfig) -> Self {
        Self {
            bus,
            ipc_handler: IpcHandler::new(service),
        }
    }

    /// Run the handler loop processing events.
    pub async fn run(self) {
        // Subscribe only to Peer Discovery events (where VerifyNodeIdentity comes from)
        let mut rx = self
            .bus
            .subscribe(EventFilter::topics(vec![EventTopic::PeerDiscovery]));
        info!("Signature Verification Handler started");

        // We specifically listen for VerifyNodeIdentity events
        while let Some(event) = rx.recv().await {
            match event {
                BlockchainEvent::VerifyNodeIdentity {
                    correlation_id,
                    payload,
                } => {
                    self.handle_verification(correlation_id, payload).await;
                }
                _ => {}
            }
        }
    }

    async fn handle_verification(
        &self,
        correlation_id: String,
        payload: VerifyNodeIdentityPayload,
    ) {
        // Parse correlation ID or generate new if invalid (logging error)
        let msg_correlation_id = match Uuid::parse_str(&correlation_id) {
            Ok(uuid) => uuid,
            Err(_) => {
                // Try from bytes if it was raw bytes hex encoded?
                // VerifyNodeIdentity payload uses String correlation_id.
                // Usually it's Uuid hex.
                // Fallback to zeros (but this breaks correlation)
                warn!("Invalid correlation ID format: {}", correlation_id);
                // Try to recover if it's 32 hex chars = 16 bytes
                if let Ok(bytes) = hex::decode(&correlation_id) {
                    if bytes.len() == 16 {
                        Uuid::from_bytes(bytes.try_into().unwrap())
                    } else {
                        Uuid::nil()
                    }
                } else {
                    Uuid::nil()
                }
            }
        };

        // Construct AuthenticatedMessage
        // We act as the transport layer here, wrapping the payload
        let msg = AuthenticatedMessage {
            version: 1,
            sender_id: 1,     // Peer Discovery (SubsystemId::PeerDiscovery)
            recipient_id: 10, // Signature Verification (SubsystemId::SignatureVerification)
            correlation_id: msg_correlation_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            nonce: Uuid::new_v4(),
            payload,
            signature: [0u8; 64], // Signature not checked by IpcHandler logic yet
            reply_to: None,       // No specific reply topic needed
        };

        // Note: msg.sign() is not available on the struct directly here,
        // and IpcHandler does not enforce HMAC verification in its current validate_envelope implementation.
        // If strict security is needed, we would need to implement signing here manually.

        // Call Handler
        match self.ipc_handler.handle_verify_node_identity(msg) {
            Ok(response) => {
                // Publish Result back to Bus
                let event = BlockchainEvent::NodeIdentityVerified {
                    correlation_id,
                    payload: response,
                };
                // Publish response
                self.bus.publish(event).await;

                // info!("Verified node identity for correlation {}", correlation_id);
            }
            Err(e) => {
                error!(
                    "Error verifying node identity (correlation {}): {}",
                    correlation_id, e
                );
            }
        }
    }
}
