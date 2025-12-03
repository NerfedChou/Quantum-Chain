//! # Block Storage Engine (qc-02)
//!
//! The Block Storage subsystem is the authoritative persistence layer for all
//! blockchain data. It implements the V2.3 **Stateful Assembler** pattern.
//!
//! ## Architecture (V2.3 Choreography Pattern)
//!
//! Block Storage does NOT receive a pre-assembled package. Instead, it subscribes
//! to THREE independent event streams and assembles them:
//!
//! ```text
//! Consensus (8) ────BlockValidated────→ ┐
//!                                        │
//! Tx Indexing (3) ──MerkleRootComputed──→├──→ Block Storage (2)
//!                                        │    [Stateful Assembler]
//! State Mgmt (4) ───StateRootComputed───→┘
//!                                        ↓
//!                             [Atomic Write when all 3 present]
//! ```
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Description |
//! |----|-----------|-------------|
//! | 1 | Sequential Blocks | Parent block must exist for height > 0 |
//! | 2 | Disk Space Safety | Writes fail if disk < 5% available |
//! | 3 | Data Integrity | Checksum verified on every read |
//! | 4 | Atomic Writes | All or nothing - no partial writes |
//! | 5 | Finalization Monotonicity | Finalization cannot regress |
//! | 6 | Genesis Immutability | Genesis hash never changes |
//! | 7 | Assembly Timeout | Incomplete assemblies purged after 30s |
//! | 8 | Bounded Assembly Buffer | Max 1000 pending assemblies |
//!
//! ## Crate Structure (Hexagonal Architecture)
//!
//! - `domain/` - Pure domain logic (entities, value objects, services)
//! - `ports/` - Port traits (inbound API, outbound SPI)
//! - `service.rs` - Application service implementing the API
//! - `ipc/` - IPC message handlers and security boundaries
//! - `bus/` - Event bus adapter for V2.3 Choreography
//!
//! ## Usage
//!
//! ```ignore
//! use qc_02_block_storage::{BlockStorageService, StorageConfig};
//!
//! // Create service with in-memory adapters
//! let config = StorageConfig::default();
//! let service = BlockStorageService::new_in_memory(config);
//!
//! // Write a block
//! let hash = service.write_block(block, merkle_root, state_root)?;
//!
//! // Read a block
//! let stored = service.read_block(&hash)?;
//! ```

pub mod bus;
pub mod domain;
pub mod ipc;
pub mod ports;
pub mod service;

// Re-export key types for convenience
pub use domain::assembler::{AssemblyConfig, BlockAssemblyBuffer, PendingBlockAssembly};
pub use domain::entities::{BlockIndex, BlockIndexEntry, StoredBlock};
pub use domain::errors::StorageError;
pub use domain::value_objects::{KeyPrefix, StorageConfig, TransactionLocation};
pub use ports::inbound::BlockStorageApi;
pub use ports::outbound::{
    BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};
pub use service::BlockStorageService;

// Re-export IPC types
pub use ipc::payloads::*;
pub use ipc::{AuthenticatedMessage, BlockStorageHandler, EnvelopeError, EnvelopeValidator};

// Re-export Bus types
pub use bus::{event_types, BlockStorageBusAdapter};
