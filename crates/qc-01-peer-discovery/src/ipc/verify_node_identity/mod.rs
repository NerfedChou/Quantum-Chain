//! # Verify Node Identity Request
//!
//! Outbound request to Subsystem 10 (Signature Verification) for DDoS defense.
//!
//! ## Flow (IPC-MATRIX.md lines 42-51, 94-100)
//!
//! ```text
//! External Peer ──BootstrapRequest──→ [Peer Discovery (1)]
//!                                            │
//!                                            ↓ stage in pending_verification
//!                                            │
//!                                     VerifyNodeIdentityRequest ──→ [Signature Verification (10)]
//!                                            │
//!                                            ← NodeIdentityVerificationResult
//!                                            │
//!                                            ↓ if identity_valid: promote to routing table
//!                                              else: reject peer
//! ```
//!
//! ## Security
//!
//! - This request can ONLY be sent BY Subsystem 1 (Peer Discovery)
//! - This request can ONLY be received BY Subsystem 10 (Signature Verification)
//! - Per Architecture.md v2.2, payload contains NO identity fields (envelope authority)

// Semantic submodules
mod request;

// Re-export public API
pub use request::VerifyNodeIdentityRequest;

#[cfg(test)]
mod tests;
