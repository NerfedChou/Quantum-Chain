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

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items
// Additional allows to match CI configuration
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::manual_strip)]
#![allow(clippy::unnecessary_to_owned)]
#![allow(clippy::expect_fun_call)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::repeat_once)]
// TODO(TECH-DEBT): Bridge loop in main.rs has 1 excessive_nesting violation requiring major refactoring
// Justification: Spawned async tasks have inherent nesting; scheduled for Phase 4 cleanup
#![allow(clippy::excessive_nesting)]

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
