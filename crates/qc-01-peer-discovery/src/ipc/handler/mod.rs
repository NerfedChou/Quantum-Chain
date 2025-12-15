//! # IPC Message Handler
//!
//! Handles incoming IPC messages with security validation.
//!
//! ## Security Integration
//!
//! This handler uses the **centralized security module** from `shared-types`
//! as mandated by Architecture.md v2.2. This ensures:
//! - Consistent security policy across all subsystems
//! - Single source of truth for HMAC and nonce validation
//! - Reduced code duplication and maintenance burden
//!
//! ## Validation Order (Architecture.md Section 3.5)
//!
//! 1. Timestamp check (bounds all operations, prevents DoS)
//! 2. Version check (before any deserialization)
//! 3. Sender check (authorization per IPC Matrix)
//! 4. Signature check (HMAC via shared-types MessageVerifier)
//! 5. Nonce check (replay prevention via shared-types NonceCache)
//! 6. Reply-to validation (forwarding attack prevention)

// Semantic submodules
mod key_provider;
mod request_handler;
mod types;

// Re-export public API
pub use key_provider::StaticKeyProvider;
pub use request_handler::IpcHandler;
pub use types::{CorrelationId, PendingRequest};

#[cfg(test)]
mod tests;
