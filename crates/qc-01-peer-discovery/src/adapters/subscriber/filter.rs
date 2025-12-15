use crate::ipc::security::SubsystemId;

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
