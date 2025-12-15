//! # Network Adapters (Phase 4)
//!
//! Production-ready network adapters for peer discovery.
//!
//! ## Adapters Provided
//!
//! - `SystemTimeSource` - Production time source using system clock
//! - `UdpNetworkSocket` - UDP-based network I/O (requires "network" feature)
//! - `TomlConfigProvider` - Config file loading (requires "network" feature)
//!
//! ## Feature Flags
//!
//! - `network` - Enables async UDP networking and config file parsing
//!
//! ## Reference
//!
//! SPEC-01-PEER-DISCOVERY.md Section 8 (Phase 4)

// Semantic submodules
/// Configuration providers
pub mod config;
/// Security validators
pub mod security;
/// Time source adapters
pub mod time;
/// Transport adapters
pub mod transport;

// Re-export public API
pub use config::StaticConfigProvider;
pub use security::{NoOpNodeIdValidator, ProofOfWorkValidator};
pub use time::SystemTimeSource;
pub use transport::{MessageType, NoOpNetworkSocket};

#[cfg(feature = "network")]
pub use config::{ConfigError, TomlConfigProvider};

#[cfg(feature = "network")]
pub use transport::UdpNetworkSocket;

#[cfg(all(feature = "network", feature = "ipc"))]
pub use security::parse_bootstrap_request;

#[cfg(test)]
mod tests;
