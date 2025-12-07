//! # Adapters Layer (Outer Hexagon)
//!
//! Adapters connect the Smart Contract subsystem to external systems.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - Adapters implement domain ports
//! - All external communication via Event Bus (EDA pattern)
//! - No direct subsystem-to-subsystem calls

pub mod access_list;
pub mod event_handler;
pub mod state_adapter;

pub use access_list::*;
pub use event_handler::*;
pub use state_adapter::*;
