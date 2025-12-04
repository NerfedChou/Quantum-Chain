//! # Ports Layer - Hexagonal Architecture Boundaries
//!
//! Defines the API contract for the Mempool subsystem.
//!
//! ## Inbound (Driving) Ports
//!
//! `MempoolApi` - Primary API for other subsystems to interact with the pool.
//! Authorization enforced per IPC-MATRIX.md.
//!
//! ## Outbound (Driven) Ports
//!
//! - `StateProvider` - Balance/nonce queries to Subsystem 4
//! - `TimeSource` - Timestamp abstraction for testability

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
