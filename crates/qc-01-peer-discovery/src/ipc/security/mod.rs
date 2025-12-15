//! # IPC Security - Authorization & Validation
//!
//! Implements security boundaries per IPC-MATRIX.md for Subsystem 1.
//!
//! ## Security Rules
//!
//! Per IPC-MATRIX.md, Peer Discovery accepts requests ONLY from:
//! - Subsystem 5 (Block Propagation) - PeerListRequest
//! - Subsystem 7 (Bloom Filters) - PeerListRequest
//! - Subsystem 13 (Light Clients) - PeerListRequest, FullNodeListRequest
//!
//! All other senders are REJECTED.
//!
//! ## Message Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations)
//! 2. Version check (before deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC)
//! 5. Nonce check (replay prevention)
//! 6. Reply-to validation (forwarding attack prevention)

// Semantic submodules
/// Authorization rules and validation logic
pub mod authorization;
/// Security error definitions
pub mod error;
/// Subsystem ID definitions
pub mod subsystem_id;

// Re-export public API
pub use authorization::AuthorizationRules;
pub use error::SecurityError;
pub use subsystem_id::SubsystemId;

#[cfg(test)]
mod tests;
