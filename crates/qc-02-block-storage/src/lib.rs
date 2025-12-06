//! # Block Storage Engine (qc-02)
//!
//! The Block Storage subsystem is the authoritative persistence layer for all
//! blockchain data. Provides domain logic for the V2.3 Stateful Assembler pattern.
//!
//! ## Architecture
//!
//! This crate provides **domain logic only**. Choreography (event buffering and
//! assembly) is handled by `node-runtime::adapters::BlockStorageAdapter`.
//!
//! ```text
//! node-runtime
//! ├── BlockStorageAdapter (choreography, event buffering)
//! │   └── calls qc-02 domain logic
//! │
//! qc-02-block-storage
//! ├── BlockStorageService (write_block, read_block, mark_finalized)
//! ├── BlockStorageApi (port trait)
//! └── Domain invariants (1-8)
//! ```
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement |
//! |----|-----------|-------------|
//! | 1 | Sequential Blocks | `check_parent_exists()` |
//! | 2 | Disk Space Safety | `check_disk_space()` |
//! | 3 | Data Integrity | `verify_block_checksum()` |
//! | 4 | Atomic Writes | `atomic_batch_write()` |
//! | 5 | Finalization Monotonicity | `metadata.on_finalized()` |
//! | 6 | Genesis Immutability | `StorageMetadata::set_genesis()` |
//! | 7 | Assembly Timeout | Enforced by node-runtime |
//! | 8 | Bounded Assembly Buffer | Enforced by node-runtime |
//!
//! ## Crate Structure (Hexagonal Architecture)
//!
//! - `domain/` - Pure domain logic (entities, value objects, invariants)
//! - `ports/` - Port traits (inbound API, outbound SPI)
//! - `service.rs` - Application service implementing BlockStorageApi
//! - `ipc/` - IPC envelope validation and message handlers
//! - `adapters/` - External interface adapters (API Gateway)
//!
//! ## Reference
//!
//! - SPEC-02-BLOCK-STORAGE.md (specification)
//! - Architecture.md Section 5.1 (choreography pattern)
//! - IPC-MATRIX.md (sender authorization)

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod ports;
pub mod service;

// Re-export domain types
pub use domain::assembler::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};
pub use domain::entities::{BlockIndex, BlockIndexEntry, StoredBlock};
pub use domain::errors::StorageError;
pub use domain::value_objects::{KeyPrefix, StorageConfig, TransactionLocation};

// Re-export port traits
pub use ports::inbound::BlockStorageApi;
pub use ports::outbound::{
    BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};

// Re-export service
pub use service::BlockStorageService;

// Re-export IPC types
pub use ipc::payloads::*;
pub use ipc::{AuthenticatedMessage, BlockStorageHandler, EnvelopeError, EnvelopeValidator};

// Re-export API Gateway handler
pub use adapters::{ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly, handle_api_query};
