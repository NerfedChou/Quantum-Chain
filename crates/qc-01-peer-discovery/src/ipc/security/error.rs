/// Security errors that can occur during IPC processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// Message version is unsupported.
    UnsupportedVersion {
        /// The version that was received.
        received: u16,
        /// Minimum supported version.
        min_supported: u16,
        /// Maximum supported version.
        max_supported: u16,
    },
    /// Message timestamp is outside valid window.
    TimestampOutOfRange {
        /// The timestamp from the message.
        timestamp: u64,
        /// Current time.
        now: u64,
    },
    /// Sender is not authorized for this request type.
    UnauthorizedSender {
        /// The sender's subsystem ID.
        sender_id: u8,
        /// List of allowed sender IDs.
        allowed_senders: &'static [u8],
    },
    /// Message signature is invalid.
    InvalidSignature,

    /// Reply-to subsystem doesn't match sender (forwarding attack).
    ReplyToMismatch {
        /// The sender's subsystem ID.
        sender_id: u8,
        /// The reply_to subsystem ID.
        reply_to_subsystem: u8,
    },
    /// Unknown subsystem ID.
    UnknownSubsystem {
        /// The unknown ID.
        id: u8,
    },
    /// Missing required reply_to for request.
    MissingReplyTo,
    /// Nonce replay detected (UUID-based from shared-types).
    ReplayDetectedUuid {
        /// The replayed nonce.
        nonce: uuid::Uuid,
    },
}

impl SecurityError {
    /// Convert from shared-types VerificationResult.
    ///
    /// Bridges the centralized `MessageVerifier` result type to subsystem-specific
    /// error types. Callers must verify `!result.is_valid()` before calling.
    ///
    /// Reference: Architecture.md Section 3.5 (Centralized Security Module)
    pub fn from_verification_result(result: shared_types::envelope::VerificationResult) -> Self {
        use shared_types::envelope::VerificationResult;
        match result {
            VerificationResult::Valid => {
                // SAFETY: Precondition violated - callers MUST check !result.is_valid() before conversion.
                // Using unreachable!() instead of panic!() for clearer intent and potential optimization.
                // If this is reached, the caller has a bug in their validation logic.
                unreachable!(
                    "SecurityError::from_verification_result() called with Valid result. \
                     Caller must verify !result.is_valid() before conversion."
                )
            }
            VerificationResult::UnsupportedVersion {
                received,
                supported,
            } => SecurityError::UnsupportedVersion {
                received,
                min_supported: supported,
                max_supported: supported,
            },
            VerificationResult::TimestampOutOfRange { timestamp, now } => {
                SecurityError::TimestampOutOfRange { timestamp, now }
            }
            VerificationResult::ReplayDetected { nonce } => {
                SecurityError::ReplayDetectedUuid { nonce }
            }
            VerificationResult::InvalidSignature => SecurityError::InvalidSignature,
            VerificationResult::ReplyToMismatch {
                reply_to_subsystem,
                sender_id,
            } => SecurityError::ReplyToMismatch {
                sender_id,
                reply_to_subsystem,
            },
        }
    }
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion {
                received,
                min_supported,
                max_supported,
            } => write!(
                f,
                "unsupported protocol version {} (supported: {}-{})",
                received, min_supported, max_supported
            ),
            Self::TimestampOutOfRange { timestamp, now } => {
                write!(
                    f,
                    "timestamp {} is outside valid window (now: {})",
                    timestamp, now
                )
            }
            Self::UnauthorizedSender {
                sender_id,
                allowed_senders,
            } => {
                write!(
                    f,
                    "unauthorized sender {} (allowed: {:?})",
                    sender_id, allowed_senders
                )
            }
            Self::InvalidSignature => write!(f, "message signature is invalid"),

            Self::ReplayDetectedUuid { nonce } => write!(f, "replay detected for nonce {}", nonce),
            Self::ReplyToMismatch {
                sender_id,
                reply_to_subsystem,
            } => write!(
                f,
                "reply_to subsystem {} doesn't match sender {}",
                reply_to_subsystem, sender_id
            ),
            Self::UnknownSubsystem { id } => write!(f, "unknown subsystem ID: {}", id),
            Self::MissingReplyTo => write!(f, "missing reply_to for request message"),
        }
    }
}

impl std::error::Error for SecurityError {}
