//! # Domain Module
//!
//! Core domain types for Cross-Chain Communication.
//!
//! Reference: SPEC-15 Section 2

pub mod entities;
pub mod errors;
pub mod invariants;
pub mod secure_secret;
pub mod value_objects;

pub use entities::*;
pub use errors::*;
pub use invariants::*;
pub use secure_secret::SecureSecret;
pub use value_objects::*;
