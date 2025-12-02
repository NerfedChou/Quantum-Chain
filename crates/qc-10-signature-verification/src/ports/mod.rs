//! # Ports Layer
//!
//! Trait definitions for the hexagonal architecture.
//! - **Inbound (Driving)**: API that external callers use
//! - **Outbound (Driven)**: Dependencies this subsystem needs

pub mod inbound;
pub mod outbound;
