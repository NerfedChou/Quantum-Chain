//! # Event Bus Adapter
//!
//! Implements the V2.3 Choreography pattern for Peer Discovery.
//!
//! This adapter bridges the domain layer with the shared-bus crate,
//! handling event publishing and subscription with proper security.

pub mod publisher;
pub mod subscriber;

pub use publisher::*;
pub use subscriber::*;
