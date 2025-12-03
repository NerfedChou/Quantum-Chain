//! IPC Message Handlers for Block Storage
//!
//! This module implements the security boundaries and message handling
//! per IPC-MATRIX.md and Architecture.md v2.3.
//!
//! ## Security Model
//!
//! - All messages wrapped in `AuthenticatedMessage<T>` envelope
//! - Envelope `sender_id` is the SOLE source of truth for identity
//! - Reply-to validation prevents forwarding attacks
//! - Nonce tracking prevents replay attacks

pub mod envelope;
pub mod handlers;
pub mod payloads;

pub use envelope::{AuthenticatedMessage, EnvelopeError, EnvelopeValidator};
pub use handlers::BlockStorageHandler;
pub use payloads::*;
