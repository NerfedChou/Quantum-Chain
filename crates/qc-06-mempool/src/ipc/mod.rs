//! IPC layer for the Mempool subsystem.
//!
//! Implements security boundaries and message handling per IPC-MATRIX.md.

pub mod handler;
pub mod payloads;
pub mod security;

pub use handler::*;
pub use payloads::*;
pub use security::*;
