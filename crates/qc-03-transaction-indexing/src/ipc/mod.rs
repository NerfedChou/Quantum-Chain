//! # IPC Layer
//!
//! Inter-Process Communication handlers and payloads for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 4: Event Schema (EDA)
//! - Section 4.1: Incoming Event Subscriptions
//! - Section 4.2: Incoming Request Payloads
//! - Section 4.3: Outgoing Event Publications
//!
//! ## Security (Envelope-Only Identity)
//!
//! All payloads contain NO identity fields. Sender identity is derived
//! SOLELY from the AuthenticatedMessage envelope's sender_id field.

pub mod handler;
pub mod payloads;

pub use handler::*;
pub use payloads::*;
