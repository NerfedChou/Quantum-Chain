//! # Ports Layer - Hexagonal Architecture Boundaries
//!
//! This module defines the port interfaces (traits) for the Peer Discovery subsystem.
//!
//! ## Architecture
//!
//! Per SPEC-01-PEER-DISCOVERY.md Section 3:
//! - **Driving Ports (Inbound):** APIs this subsystem exposes to consumers
//! - **Driven Ports (Outbound):** SPIs this subsystem requires from adapters
//!
//! ## Security
//!
//! All ports enforce the security policies from Architecture.md v2.2:
//! - DDoS Edge Defense via bounded staging
//! - Memory Bomb Defense via Tail Drop
//! - Eclipse Attack Defense via Eviction-on-Failure

pub mod inbound;
pub mod outbound;

pub use inbound::{PeerDiscoveryApi, VerificationHandler};
pub use outbound::{ConfigProvider, NetworkError, NetworkSocket, NodeIdValidator, TimeSource};
