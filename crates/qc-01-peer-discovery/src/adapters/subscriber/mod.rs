//! # Event Subscriber Adapter
//!
//! Subscribes to events from other subsystems via the shared event bus.
//!
//! ## Events Subscribed (per IPC-MATRIX.md)
//!
//! - From Subsystem 10 (Signature Verification): `NodeIdentityVerificationResult`
//!
//! This allows Peer Discovery to verify node identities at the network edge
//! for DDoS defense.
//!
//! ## EDA Pattern (Architecture.md v2.3)
//!
//! This adapter implements the Event-Driven Architecture pattern:
//! - Receives events from the shared bus
//! - Validates sender authorization per IPC-MATRIX
//! - Routes to the domain service for processing
//! - Emits resulting events via the publisher

mod filter;
mod handler;
mod types;

pub use filter::SubscriptionFilter;
pub use handler::{validate_identity_result_sender, EventHandler, PeerDiscoveryEventSubscriber};
pub use types::{NodeIdentityVerificationResult, SubscriptionError, VerificationOutcome};

#[cfg(test)]
mod tests;
