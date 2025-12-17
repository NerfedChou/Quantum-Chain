//! # Authenticated Message
//!
//! Core message struct and Topic for envelope routing.

use std::time::{SystemTime, UNIX_EPOCH};

/// Protocol version for envelope compatibility
pub const PROTOCOL_VERSION: u8 = 1;
pub const MIN_SUPPORTED_VERSION: u8 = 1;
pub const MAX_SUPPORTED_VERSION: u8 = 1;

/// Maximum message age in seconds (60 seconds per Architecture.md)
pub const MAX_MESSAGE_AGE_SECS: u64 = 60;

/// Authenticated message envelope per Architecture.md Section 3.2
///
/// This is the ONLY way subsystems communicate. The envelope provides:
/// - Authentication via HMAC signature
/// - Replay prevention via nonce + timestamp
/// - Request/response correlation via correlation_id
/// - Routing via sender_id + recipient_id
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedMessage<T> {
    /// Protocol version for backward compatibility
    pub version: u8,
    /// Unique correlation ID for request/response matching (UUID v4 bytes)
    pub correlation_id: [u8; 16],
    /// Topic to send response to (None for events/responses)
    pub reply_to: Option<Topic>,
    /// Sender subsystem ID - THE source of truth for identity
    pub sender_id: u8,
    /// Recipient subsystem ID
    pub recipient_id: u8,
    /// Unix timestamp in seconds
    pub timestamp: u64,
    /// Unique nonce for replay prevention
    pub nonce: u64,
    /// HMAC-SHA256 signature over all fields
    pub signature: [u8; 32],
    /// The actual payload
    pub payload: T,
}

/// Topic for event routing
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    pub subsystem_id: u8,
    pub channel: String,
}

impl Topic {
    pub fn new(subsystem_id: u8, channel: impl Into<String>) -> Self {
        Self {
            subsystem_id,
            channel: channel.into(),
        }
    }

    pub fn to_topic_string(&self) -> String {
        format!("subsystem.{}.{}", self.subsystem_id, self.channel)
    }
}

impl<T> AuthenticatedMessage<T> {
    /// Create a new message (for sending)
    pub fn new(sender_id: u8, recipient_id: u8, payload: T, reply_to: Option<Topic>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            correlation_id: generate_correlation_id(),
            reply_to,
            sender_id,
            recipient_id,
            timestamp: current_timestamp(),
            nonce: generate_nonce(),
            signature: [0u8; 32], // To be signed
            payload,
        }
    }

    /// Create a response to a request (preserves correlation_id)
    pub fn response(
        original: &AuthenticatedMessage<impl std::any::Any>,
        sender_id: u8,
        payload: T,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            correlation_id: original.correlation_id,
            reply_to: None, // Responses don't need reply_to
            sender_id,
            recipient_id: original.sender_id, // Send back to requester
            timestamp: current_timestamp(),
            nonce: generate_nonce(),
            signature: [0u8; 32], // To be signed
            payload,
        }
    }
}

/// Get current Unix timestamp
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Generate a correlation ID combining timestamp and counter.
pub fn generate_correlation_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    let ts = current_timestamp();
    id[0..8].copy_from_slice(&ts.to_le_bytes());
    id[8..16].copy_from_slice(&generate_nonce().to_le_bytes());
    id
}

/// Generate a unique nonce using timestamp and atomic counter.
fn generate_nonce() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    current_timestamp().wrapping_mul(1_000_000) + count
}
