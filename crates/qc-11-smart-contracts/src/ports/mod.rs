//! # Ports Layer (Middle Hexagon)
//!
//! Trait definitions for Smart Contract execution.
//! These are the interfaces between the domain and the outside world.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - **Driving Ports (Inbound)**: `SmartContractApi`, `HtlcExecutor`
//! - **Driven Ports (Outbound)**: `StateAccess`, `SignatureVerifier`
//! - No concrete implementations in this module

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
