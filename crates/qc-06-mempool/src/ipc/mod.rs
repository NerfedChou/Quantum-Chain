//! # IPC Layer - Mempool Subsystem
//!
//! Security boundaries and message handling per IPC-MATRIX.md v2.3.
//!
//! ## Security Architecture
//!
//! Uses centralized `shared-types::security` module for:
//! - HMAC signature validation
//! - Nonce/replay prevention
//! - Timestamp bounds checking
//!
//! ## Authorization Rules
//!
//! See `security.rs` for per-message sender authorization.

pub mod handler;
pub mod payloads;
pub mod security;

pub use handler::*;
pub use payloads::*;
pub use security::*;
