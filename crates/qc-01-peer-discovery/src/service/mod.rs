//! # Peer Discovery Service
//!
//! High-level service implementing the `PeerDiscoveryApi` and `VerificationHandler` ports.
//!
//! This service wraps the domain `RoutingTable` and provides a clean API
//! for consumers, hiding the internal complexity of time management and
//! the verification workflow.
//!
//! ## EDA Integration
//!
//! The service implements `VerificationHandler` to receive verification events
//! from Subsystem 10 via the event bus.
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Section 3.1

// Semantic submodules
mod api;
mod core;
mod events;
mod maintenance;

// Re-export public API
pub use core::PeerDiscoveryService;

#[cfg(test)]
mod tests;
