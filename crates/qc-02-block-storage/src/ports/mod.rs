//! # Ports Layer
//!
//! Defines the port traits for the Block Storage subsystem.
//!
//! ## Hexagonal Architecture
//!
//! - `inbound.rs` - Driving ports (API exposed to other subsystems)
//! - `outbound.rs` - Driven ports (dependencies required by the service)

pub mod inbound;
pub mod outbound;
