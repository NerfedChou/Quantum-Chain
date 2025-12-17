//! # Envelope Module
//!
//! AuthenticatedMessage envelope implementation per Architecture.md v2.3.
//!
//! ## Modules
//!
//! - `message`: AuthenticatedMessage struct and Topic
//! - `validator`: EnvelopeValidator for security checks
//! - `errors`: EnvelopeError enum
//! - `security`: HMAC signature and replay protection

mod errors;
mod message;
mod security;
#[cfg(test)]
mod tests;
mod validator;

pub use errors::EnvelopeError;
pub use message::{
    current_timestamp, generate_correlation_id, AuthenticatedMessage, Topic, MAX_MESSAGE_AGE_SECS,
    MAX_SUPPORTED_VERSION, MIN_SUPPORTED_VERSION, PROTOCOL_VERSION,
};
pub use security::{compute_message_signature, subsystem_ids, SignatureContext};
pub use validator::EnvelopeValidator;
