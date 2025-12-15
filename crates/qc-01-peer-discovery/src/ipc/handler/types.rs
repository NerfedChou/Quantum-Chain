//! IPC Handler types.

/// Correlation ID for request/response tracking.
pub type CorrelationId = [u8; 16];

/// Pending request awaiting response.
#[derive(Debug, Clone)]
pub struct PendingRequest {
    /// When the request was sent.
    pub sent_at: u64,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// The correlation ID.
    pub correlation_id: CorrelationId,
}
