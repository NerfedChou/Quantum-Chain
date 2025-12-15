//! # Event Publisher Adapter
//!
//! Publishes peer discovery events to the shared event bus.
//!
//! ## Events Published
//!
//! Per SPEC-01 Section 4.1:
//! - `PeerConnected` - When a peer is successfully added to routing table
//! - `PeerDisconnected` - When a peer is removed
//! - `PeerBanned` - When a peer is banned
//! - `BootstrapCompleted` - When bootstrap process finishes
//! - `RoutingTableWarning` - When health issues are detected

// Semantic submodules
/// Event builder logic
pub mod builder;
/// Mock implementations for testing
pub mod mocks;
/// Trait definitions
pub mod traits;

// Re-export public API
pub use builder::EventBuilder;
pub use mocks::{
    InMemoryEventPublisher, InMemoryVerificationPublisher, NoOpEventPublisher,
    NoOpVerificationPublisher,
};
pub use traits::{PeerDiscoveryEventPublisher, VerificationRequestPublisher};

#[cfg(test)]
mod tests;
