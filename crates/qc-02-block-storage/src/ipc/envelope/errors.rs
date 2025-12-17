//! # Envelope Errors
//!
//! Error types for envelope validation.

/// Envelope validation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeError {
    /// Protocol version not supported
    UnsupportedVersion { version: u8, min: u8, max: u8 },
    /// Message timestamp too old
    MessageExpired { age_secs: u64, max_age: u64 },
    /// Message timestamp in the future
    MessageFromFuture { timestamp: u64, now: u64 },
    /// Nonce already used (replay attack)
    NonceReused { nonce: u64 },
    /// Signature verification failed
    InvalidSignature,
    /// Sender not authorized for this operation
    UnauthorizedSender { sender: u8, allowed: Vec<u8> },
    /// Reply-to subsystem doesn't match sender (forwarding attack)
    ReplyToMismatch { sender: u8, reply_to: u8 },
    /// Recipient doesn't match our subsystem
    WrongRecipient { expected: u8, actual: u8 },
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion { version, min, max } => {
                write!(
                    f,
                    "Unsupported version {} (supported: {}-{})",
                    version, min, max
                )
            }
            Self::MessageExpired { age_secs, max_age } => {
                write!(f, "Message expired: age={}s, max={}s", age_secs, max_age)
            }
            Self::MessageFromFuture { timestamp, now } => {
                write!(
                    f,
                    "Message from future: timestamp={}, now={}",
                    timestamp, now
                )
            }
            Self::NonceReused { nonce } => {
                write!(f, "Nonce already used: {}", nonce)
            }
            Self::InvalidSignature => write!(f, "Invalid signature"),
            Self::UnauthorizedSender { sender, allowed } => {
                write!(f, "Unauthorized sender {}, allowed: {:?}", sender, allowed)
            }
            Self::ReplyToMismatch { sender, reply_to } => {
                write!(
                    f,
                    "Reply-to mismatch: sender={}, reply_to={}",
                    sender, reply_to
                )
            }
            Self::WrongRecipient { expected, actual } => {
                write!(
                    f,
                    "Wrong recipient: expected={}, actual={}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for EnvelopeError {}
