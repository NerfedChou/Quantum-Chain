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
//! - `compression` - Dictionary-based Zstd compression (Phase 3)
//! - `repair` - Self-healing index for disaster recovery (Phase 4)
//! - `mmr` - Merkle Mountain Range for light client proofs (Phase 3)
//! - `pruning` - Smart pruning with anchor blocks (SPEC 5.2)
//! - `snapshot` - State snapshot export/import (SPEC 6.1)
//! - `metrics` - Compaction and storage metrics (SPEC 4.3)

pub mod assembler;
pub mod compression;
pub mod entities;
pub mod errors;
pub mod metrics;
pub mod mmr;
pub mod pruning;
pub mod repair;
pub mod snapshot;
pub mod value_objects;
