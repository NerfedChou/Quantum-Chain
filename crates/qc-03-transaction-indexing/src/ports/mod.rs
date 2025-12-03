//! # Ports Layer
//!
//! Hexagonal architecture ports (interfaces) for the Transaction Indexing subsystem.
//!
//! ## SPEC-03 Reference
//!
//! - Section 3.1: Driving Ports (Inbound API)
//! - Section 3.2: Driven Ports (Outbound SPI)
//!
//! ## Hexagonal Architecture
//!
//! - **Driving Ports (Inbound)**: APIs consumed by adapters (handlers, CLI, etc.)
//! - **Driven Ports (Outbound)**: SPIs implemented by adapters (storage, crypto, etc.)

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
