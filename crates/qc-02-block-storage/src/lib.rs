//! # Block Storage Engine (qc-02)
//!
//! The Block Storage subsystem is the authoritative persistence layer for all
//! blockchain data. Provides domain logic for the V2.3 Stateful Assembler pattern.
//!
//! ## Architecture
//!
//! This crate provides **domain logic only**. The runtime calls into this crate's
//! domain APIs, including periodic garbage collection for assembly timeouts.
//!
//! ```text
//! node-runtime
//! ├── BlockStorageAdapter (choreography, event routing)
//! │   └── calls qc-02 domain logic
//! │   └── calls gc_expired_assemblies() periodically (every 5s)
//! │
//! qc-02-block-storage
//! ├── BlockStorageService (write_block, read_block, mark_finalized)
//! ├── BlockAssemblyBuffer (stateful assembler with timeout/buffer logic)
//! ├── BlockStorageApi (port trait)
//! └── Domain invariants (1-8)
//! ```
//!
//! ## Domain Invariants (SPEC-02 Section 2.6)
//!
//! | ID | Invariant | Enforcement | Location |
//! |----|-----------|-------------|----------|
//! | 1 | Sequential Blocks | `check_parent_exists()` | service.rs |
//! | 2 | Disk Space Safety | `check_disk_space()` | service.rs |
//! | 3 | Data Integrity | `verify_block_checksum()` | service.rs |
//! | 4 | Atomic Writes | `atomic_batch_write()` | service.rs |
//! | 5 | Finalization Monotonicity | `metadata.on_finalized()` | entities.rs |
//! | 6 | Genesis Immutability | `StorageMetadata::set_genesis()` | entities.rs |
//! | 7 | Assembly Timeout | `gc_expired()` logic in crate | assembler.rs (runtime calls periodically) |
//! | 8 | Bounded Assembly Buffer | `enforce_max_pending()` | assembler.rs |
//!
//! **Note on Invariants 7-8:** The timeout and buffer limit logic is fully implemented
//! in `domain/assembler.rs`. The runtime is responsible for calling `gc_expired_assemblies()`
//! at regular intervals (recommended: every 5 seconds). This separation follows DDD
//! principles where domain logic is pure and side-effect-free.
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

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items

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
pub use adapters::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly,
};
