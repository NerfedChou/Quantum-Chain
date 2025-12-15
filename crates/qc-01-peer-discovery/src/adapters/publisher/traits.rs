use crate::ipc::payloads::{PeerDiscoveryEventPayload, PeerListResponsePayload};
use crate::ipc::VerifyNodeIdentityRequest;

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
