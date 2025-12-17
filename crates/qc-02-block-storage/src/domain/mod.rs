//! # Domain Layer
//!
//! Pure domain logic for the Block Storage subsystem.
//!
//! ## Modules
//!
//! - `storage` - Core storage entities (StoredBlock, BlockIndex, StorageMetadata)
//! - `assembler` - Stateful Assembler for V2.3 Choreography
//! - `integrity` - Error types and data integrity checking
//! - `compression` - Dictionary-based Zstd compression (requires `compression` feature)
//! - `metrics` - Compaction and storage metrics
//! - `mmr` - Merkle Mountain Range for light client proofs
//! - `pruning` - Smart pruning with anchor blocks
//! - `repair` - Self-healing index for disaster recovery
//! - `snapshot` - State snapshot export/import
//! - `types` - Configuration and value objects

pub mod assembler;
#[cfg(feature = "compression")]
pub mod compression;
pub mod integrity;
pub mod metrics;
pub mod mmr;
pub mod pruning;
pub mod repair;
pub mod snapshot;
pub mod storage;
pub mod types;

// Re-export core types for convenience
pub use assembler::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};
pub use integrity::{FSError, KVStoreError, SerializationError, StorageError};
pub use storage::{BlockIndex, BlockIndexEntry, StorageMetadata, StoredBlock, Timestamp};
pub use types::{CompactionStrategy, KeyPrefix, StorageConfig, TransactionLocation};

// Legacy aliases for backward compatibility
pub mod errors {
    pub use super::integrity::*;
}

pub mod value_objects {
    pub use super::types::*;
}
