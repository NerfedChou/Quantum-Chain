//! # IPC Module
//!
//! IPC Message Handlers for Block Storage per IPC-MATRIX.md and Architecture.md v2.3.
//!
//! ## Security Model
//!
//! - All messages wrapped in `AuthenticatedMessage<T>` envelope
//! - Envelope `sender_id` is the SOLE source of truth for identity
//! - Reply-to validation prevents forwarding attacks
//! - Nonce tracking prevents replay attacks
//!
//! ## Modules
//!
//! - `envelope/`: AuthenticatedMessage validation and security
//! - `handler/`: BlockStorageHandler for IPC message handling
//! - `payloads/`: IPC payload types (events, requests, responses)
//! - `security/`: Subsystem-level IPC security

pub mod envelope;
pub mod handler;
pub mod payloads;
pub mod security;

pub use envelope::{AuthenticatedMessage, EnvelopeError, EnvelopeValidator};
pub use handler::BlockStorageHandler;
pub use payloads::*;
