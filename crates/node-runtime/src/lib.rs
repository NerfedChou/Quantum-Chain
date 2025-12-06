//! # Node Runtime Library
//!
//! This library exposes the internal modules of the node runtime for testing.
//! The main entry point is the `main.rs` binary.
//!
//! ## Architectural Patterns
//!
//! - **EDA (Event-Driven Architecture)**: Subsystems communicate via Event Bus only
//! - **DDD (Domain-Driven Design)**: Each subsystem owns its domain logic
//! - **Hexagonal Architecture**: Ports define contracts, Adapters implement them
//! - **Plug-and-Play**: Subsystems can be enabled/disabled via configuration

pub mod adapters;
pub mod container;
pub mod genesis;
pub mod handlers;
pub mod registry;
pub mod wiring;

// Re-export registry types for easy access
pub use registry::{
    Subsystem, SubsystemConfig, SubsystemError, SubsystemId, SubsystemRegistry, SubsystemStatus,
};
