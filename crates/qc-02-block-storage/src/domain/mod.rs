//! # Domain Layer
//!
//! Pure domain logic for the Block Storage subsystem.
//! This layer contains NO external dependencies - only pure Rust types and logic.
//!
//! ## Modules
//!
//! - `entities` - Core domain entities (StoredBlock, BlockIndex, StorageMetadata)
//! - `assembler` - Stateful Assembler for V2.3 Choreography
//! - `value_objects` - Configuration and immutable value types
//! - `errors` - Domain error types

pub mod assembler;
pub mod entities;
pub mod errors;
pub mod value_objects;
