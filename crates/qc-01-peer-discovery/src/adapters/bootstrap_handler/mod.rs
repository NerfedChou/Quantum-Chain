//! # Bootstrap Handler Adapter
//!
//! Handles incoming `BootstrapRequest` from external peers.
//!
//! ## DDoS Defense Flow
//!
//! ```text
//! External Peer ──BootstrapRequest──→ [BootstrapHandler]
//!                                           │
//!                                           ├─ 1. Validate PoW (anti-Sybil)
//!                                           │
//!                                           ├─ 2. Check bans/subnet limits
//!                                           │
//!                                           ├─ 3. Stage in pending_verification
//!                                           │
//!                                           └─ 4. Publish VerifyNodeIdentityRequest ──→ Subsystem 10
//! ```
//!
//! The handler does NOT add peers directly to the routing table.
//! It stages them and awaits `NodeIdentityVerificationResult` from Subsystem 10.

// Semantic submodules
mod handler;
mod security;

// Re-export public API
pub use handler::{BootstrapHandler, BootstrapHandlerConfig};
pub use security::ProofOfWork;

#[cfg(test)]
mod tests;
