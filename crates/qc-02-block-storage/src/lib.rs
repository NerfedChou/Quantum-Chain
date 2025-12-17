//! # Block Storage Engine (qc-02)
//!
//! The Block Storage subsystem is the authoritative persistence layer for all
//! blockchain data. Provides domain logic for the V2.3 Stateful Assembler pattern.
//!
//! ## Feature Flags
//!
//! - `ipc` - IPC integration (event bus, envelope validation)
//! - `api` - API Gateway (JSON responses)
//! - `compression` - ZSTD dictionary compression
//! - `locking` - Process-level flock
//! - `tracing-log` - Logging integration
//! - `full` - All features enabled (default)
//!
//! ## Architecture
//!
//! This crate provides **domain logic only**. The runtime calls into this crate's
//! domain APIs, including periodic garbage collection for assembly timeouts.
//!
//! ## Domain Invariants (SPEC-02 Section 2.6)
//!
//! | ID | Invariant | Enforcement |
//! |----|-----------|-------------|
//! | 1 | Sequential Blocks | `check_parent_exists()` |
//! | 2 | Disk Space Safety | `check_disk_space()` |
//! | 3 | Data Integrity | `verify_block_checksum()` |
//! | 4 | Atomic Writes | `atomic_batch_write()` |
//! | 5 | Finalization Monotonicity | `metadata.on_finalized()` |
//! | 6 | Genesis Immutability | `StorageMetadata::set_genesis()` |
//! | 7 | Assembly Timeout | `gc_expired()` |
//! | 8 | Bounded Assembly Buffer | `enforce_max_pending()` |

// Core modules (always available)
pub mod domain;
pub mod ports;
pub mod service;

#[cfg(test)]
pub mod test_utils;

// Optional modules
pub mod adapters;
#[cfg(feature = "ipc")]
pub mod ipc;

// Re-export domain types
pub use domain::assembler::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};
pub use domain::errors::{FSError, KVStoreError, StorageError};
pub use domain::storage::{BlockIndex, BlockIndexEntry, StoredBlock};
pub use domain::value_objects::{KeyPrefix, StorageConfig, TransactionLocation};

// Re-export port traits
pub use ports::inbound::BlockStorageApi;
pub use ports::outbound::{
    BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};

// Re-export service
#[cfg(feature = "ipc")]
pub use service::subsystem_ids;
pub use service::{BlockStorageDependencies, BlockStorageService};

// Re-export IPC types (requires ipc feature)
#[cfg(feature = "ipc")]
pub use ipc::payloads::*;
#[cfg(feature = "ipc")]
pub use ipc::{AuthenticatedMessage, BlockStorageHandler, EnvelopeError, EnvelopeValidator};

// Re-export API Gateway handler (requires api feature)
#[cfg(feature = "api")]
pub use adapters::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly,
};
