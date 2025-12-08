//! # Domain Layer (Inner Hexagon)
//!
//! Pure business logic for smart contract execution.
//! NO I/O, NO async, NO external dependencies.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - This is the **inner layer** of the hexagonal architecture.
//! - All types here are pure domain concepts.
//! - Dependencies point INWARD only (adapters depend on this, not vice versa).

pub mod entities;
pub mod invariants;
pub mod services;
pub mod value_objects;

pub use entities::*;
pub use invariants::*;
pub use services::*;
pub use value_objects::*;
