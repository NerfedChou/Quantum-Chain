//! # Connection Slots Management
//!
//! Implements deterministic slot reservation with Score-Based Eviction.
//!
//! ## Design (Bitcoin Core Inspired)
//!
//! - **Outbound Slots**: Sacred - only populated by our logic
//! - **Inbound Slots**: Populated by external peers dialing us
//!
//! Reference: Bitcoin Core's `net.cpp` eviction logic

// Semantic submodules
mod config;
mod manager;
mod security;
mod types;

// Re-export public API
pub use config::ConnectionSlotsConfig;
pub use manager::ConnectionSlots;
pub use security::ConnectionInfo;
pub use types::{AcceptResult, ConnectionDirection, ConnectionStats};

#[cfg(test)]
mod tests;
