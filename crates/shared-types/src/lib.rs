//! # Shared Types Crate
//!
//! This crate contains all domain entities, IPC message types, and the
//! `AuthenticatedMessage<T>` envelope as defined in Architecture.md v2.2.
//!
//! ## Design Principles
//!
//! - **Single Source of Truth**: All cross-subsystem types are defined here.
//! - **Envelope Integrity**: The `AuthenticatedMessage<T>` is the sole wrapper
//!   for all IPC communication.
//! - **No Redundant Identity**: Payloads MUST NOT contain `requester_id` fields;
//!   the envelope's `sender_id` is authoritative.

pub mod entities;
pub mod envelope;
pub mod errors;
pub mod ipc;
pub mod security;

pub use entities::*;
pub use envelope::AuthenticatedMessage;
pub use errors::*;
pub use ipc::*;
pub use security::*;
