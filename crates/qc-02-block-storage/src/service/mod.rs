//! # Block Storage Service
//!
//! The main service implementing the Block Storage API.
//!
//! ## Architecture
//!
//! This service:
//! 1. Implements `BlockStorageApi` for read/write operations
//! 2. Implements `BlockAssemblerApi` for V2.3 choreography
//! 3. Enforces all 8 domain invariants
//! 4. Uses dependency injection for all external dependencies

mod assembler;
mod helpers;
mod storage;
#[cfg(test)]
mod tests;

use crate::domain::assembler::BlockAssemblyBuffer;
use crate::domain::errors::StorageError;
use crate::domain::storage::{BlockIndex, StorageMetadata, StoredBlock, Timestamp};
use crate::domain::storage::security::verify_block_hash_nonzero;
use crate::domain::value_objects::{KeyPrefix, StorageConfig, TransactionLocation};
use crate::ports::inbound::BlockStorageApi;
use crate::ports::outbound::{
    BatchOperation, BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};
use shared_types::{Hash, ValidatedBlock};
use std::collections::HashMap;

/// Subsystem IDs per IPC-MATRIX.md
pub mod subsystem_ids {
    pub const BLOCK_STORAGE: u8 = 2;
    pub const TRANSACTION_INDEXING: u8 = 3;
    pub const STATE_MANAGEMENT: u8 = 4;
    pub const CONSENSUS: u8 = 8;
    pub const FINALITY: u8 = 9;
}

/// The Block Storage Service.
///
/// Implements both `BlockStorageApi` (read/write operations) and `BlockAssemblerApi`
/// (V2.3 choreography event handling).
pub struct BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    /// Key-value store for persistence.
    pub(crate) kv_store: KV,
    /// Filesystem adapter for disk space checks (INVARIANT-2).
    pub(crate) fs_adapter: FS,
    /// Checksum provider for data integrity (INVARIANT-3).
    pub(crate) checksum: CS,
    /// Time source for timestamps.
    pub(crate) time_source: TS,
    /// Block serializer for StoredBlock encoding/decoding.
    pub(crate) serializer: BS,
    /// Service configuration.
    pub(crate) config: StorageConfig,
    /// Assembly buffer for V2.3 choreography (INVARIANT-7, INVARIANT-8).
    pub(crate) assembly_buffer: BlockAssemblyBuffer,
    /// In-memory block index (height -> hash).
    pub(crate) block_index: BlockIndex,
    /// In-memory storage metadata.
    pub(crate) metadata: StorageMetadata,
    /// Transaction index for Merkle proof generation (V2.3).
    pub(crate) tx_index: HashMap<Hash, TransactionLocation>,
}

/// Dependencies for BlockStorageService
pub struct BlockStorageDependencies<KV, FS, CS, TS, BS> {
    pub kv_store: KV,
    pub fs_adapter: FS,
    pub checksum: CS,
    pub time_source: TS,
    pub serializer: BS,
}

impl<KV, FS, CS, TS, BS> BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    /// Create a new Block Storage Service with the given dependencies.
    ///
    /// On construction, this will:
    /// 1. Load the block index from persistent storage
    /// 2. Load the transaction index (if `persist_transaction_index` is enabled)
    pub fn new(deps: BlockStorageDependencies<KV, FS, CS, TS, BS>, config: StorageConfig) -> Self {
        let mut service = Self {
            kv_store: deps.kv_store,
            fs_adapter: deps.fs_adapter,
            checksum: deps.checksum,
            time_source: deps.time_source,
            serializer: deps.serializer,
            config: config.clone(),
            assembly_buffer: BlockAssemblyBuffer::new(config.assembly_config.clone()),
            block_index: BlockIndex::new(),
            metadata: StorageMetadata::default(),
            tx_index: HashMap::new(),
        };

        // Load index from persistent storage (ignore errors on initial setup)
        let _ = service.load_index_from_storage();

        // Load transaction index if persistence is enabled
        if config.persist_transaction_index {
            let _ = service.load_transaction_index_from_storage();
        }

        service
    }
}
