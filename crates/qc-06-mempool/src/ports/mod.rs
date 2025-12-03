//! Ports layer for Mempool subsystem.
//!
//! Defines the hexagonal architecture port traits:
//! - Inbound (Driving) ports: API exposed to other subsystems
//! - Outbound (Driven) ports: Dependencies on external systems

pub mod inbound;
pub mod outbound;

pub use inbound::*;
pub use outbound::*;
