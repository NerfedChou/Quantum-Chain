//! # `AuthenticatedMessage` Envelope
//!
//! The universal wrapper for ALL IPC communication as mandated by Architecture.md v2.2.
//!
//! ## Security Properties
//!
//! - **Versioning**: All messages include a `version` field for forward compatibility.
//! - **Correlation**: Request/response flows use `correlation_id` and `reply_to`.
//! - **Time-Bounded Replay Prevention**: Nonces are only valid within the timestamp window.
//! - **Envelope Authority**: The `sender_id` is the sole source of truth for identity.

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};
use uuid::Uuid;

/// The topic/channel for routing responses in request/response flows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyTo {
    /// The topic name to publish the response to.
    pub topic: String,
    /// The subsystem ID that should receive the response.
    pub subsystem_id: u8,
}

/// The universal message envelope for all IPC communication.
///
/// # Architecture Compliance (v2.2)
///
/// - All IPC messages MUST be wrapped in this envelope.
/// - The `sender_id` is the ONLY source of truth for the sender's identity.
/// - Payloads MUST NOT contain redundant identity fields.
/// - Request/response flows MUST use `correlation_id` and `reply_to`.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatedMessage<T> {
    // =========================================================================
    // HEADER SECTION
    // =========================================================================
    /// Protocol version for forward compatibility.
    /// MUST be checked by deserializers before processing.
    pub version: u16,

    /// The subsystem ID of the sender. This is the SOLE source of truth
    /// for the sender's identity. Payloads MUST NOT duplicate this.
    pub sender_id: u8,

    /// The intended recipient subsystem ID.
    pub recipient_id: u8,

    /// Unique identifier for correlating request/response pairs.
    /// For requests: A newly generated UUID.
    /// For responses: The UUID from the original request.
    pub correlation_id: Uuid,

    /// Optional routing information for responses.
    /// MUST be present for request messages expecting a response.
    /// Responders MUST validate that `reply_to.subsystem_id == sender_id`.
    pub reply_to: Option<ReplyTo>,

    // =========================================================================
    // SECURITY SECTION (Time-Bounded Replay Prevention)
    // =========================================================================
    /// Unix timestamp (seconds since epoch) when the message was created.
    /// Valid window: `now - 60s <= timestamp <= now + 10s`.
    /// Messages outside this window MUST be rejected immediately.
    pub timestamp: u64,

    /// Unique nonce for replay prevention within the timestamp window.
    /// Nonces are garbage-collected after the timestamp expires.
    pub nonce: Uuid,

    /// Ed25519 signature over the serialized header + payload.
    /// Verified using the sender's public key.
    #[serde_as(as = "Bytes")]
    pub signature: [u8; 64],

    // =========================================================================
    // PAYLOAD SECTION
    // =========================================================================
    /// The actual message payload (generic over message type).
    pub payload: T,
}

impl<T> AuthenticatedMessage<T> {
    /// Current protocol version.
    pub const CURRENT_VERSION: u16 = 1;

    /// Maximum allowed clock skew for future timestamps (seconds).
    pub const MAX_FUTURE_SKEW: u64 = 10;

    /// Maximum age for valid timestamps (seconds).
    pub const MAX_AGE: u64 = 60;

    /// Duration to retain nonces in cache (2x the validity window).
    pub const NONCE_CACHE_TTL: u64 = 120;
}

/// Result of message verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationResult {
    /// Message passed all verification checks.
    Valid,
    /// Message version is unsupported.
    UnsupportedVersion { received: u16, supported: u16 },
    /// Message timestamp is outside the valid window.
    TimestampOutOfRange { timestamp: u64, now: u64 },
    /// Message nonce has been seen before (replay attack).
    ReplayDetected { nonce: Uuid },
    /// Message signature is invalid.
    InvalidSignature,
    /// The `reply_to.subsystem_id` does not match `sender_id` (forwarding attack).
    ReplyToMismatch {
        reply_to_subsystem: u8,
        sender_id: u8,
    },
}

impl VerificationResult {
    /// Returns true if the verification was successful.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, VerificationResult::Valid)
    }

    /// Returns true if the verification failed.
    #[must_use]
    pub fn is_error(&self) -> bool {
        !self.is_valid()
    }
}
