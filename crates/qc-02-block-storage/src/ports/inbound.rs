//! # Inbound Ports (Driving Ports)
//!
//! The primary API for the Block Storage subsystem.
//!
//! ## SPEC-02 Section 3.1
//!
//! These are the public APIs this library exposes to the application.

use crate::domain::entities::{StorageMetadata, StoredBlock, Timestamp};
use crate::domain::errors::StorageError;
use crate::domain::value_objects::TransactionLocation;
use shared_types::{Hash, ValidatedBlock};

/// Primary API for the Block Storage subsystem.
///
/// ## SPEC-02 Section 3.1
///
/// This trait defines all operations available to other subsystems.
/// Implementations must enforce all domain invariants.
pub trait BlockStorageApi {
    /// Write a validated block with its associated roots.
    ///
    /// ## Atomicity (INVARIANT-4)
    ///
    /// This operation is atomic. Either all data is written or none.
    ///
    /// ## Errors
    ///
    /// - `DiskFull`: Available disk space < 5% (INVARIANT-2)
    /// - `ParentNotFound`: Parent block does not exist (INVARIANT-1)
    /// - `BlockExists`: Block with this hash already stored
    /// - `BlockTooLarge`: Block exceeds maximum size limit
    fn write_block(
        &mut self,
        block: ValidatedBlock,
        merkle_root: Hash,
        state_root: Hash,
    ) -> Result<Hash, StorageError>;

    /// Read a block by its hash.
    ///
    /// ## Integrity (INVARIANT-3)
    ///
    /// Checksum is verified before returning. Corrupted data raises error.
    ///
    /// ## Errors
    ///
    /// - `BlockNotFound`: No block with this hash exists
    /// - `DataCorruption`: Checksum mismatch detected
    fn read_block(&self, hash: &Hash) -> Result<StoredBlock, StorageError>;

    /// Read a block by its height.
    ///
    /// ## Errors
    ///
    /// - `HeightNotFound`: No block at this height
    /// - `DataCorruption`: Checksum mismatch detected
    fn read_block_by_height(&self, height: u64) -> Result<StoredBlock, StorageError>;

    /// Read a range of blocks by height (for node syncing).
    ///
    /// ## Performance
    ///
    /// This is optimized for sequential reads and is the preferred
    /// method for syncing nodes that need multiple consecutive blocks.
    ///
    /// ## Parameters
    ///
    /// - `start_height`: First block height to read (inclusive)
    /// - `limit`: Maximum number of blocks to return (capped at 100)
    ///
    /// ## Returns
    ///
    /// Vector of StoredBlocks in ascending height order.
    /// May return fewer blocks than `limit` if end of chain reached.
    ///
    /// ## Errors
    ///
    /// - `HeightNotFound`: start_height does not exist
    /// - `DataCorruption`: Checksum mismatch detected in any block
    fn read_block_range(
        &self,
        start_height: u64,
        limit: u64,
    ) -> Result<Vec<StoredBlock>, StorageError>;

    /// Mark a block height as finalized.
    ///
    /// ## INVARIANT-5: Finalization Monotonicity
    ///
    /// Finalization cannot regress. Once a block is finalized, all blocks
    /// at lower heights are also considered finalized.
    ///
    /// ## Errors
    ///
    /// - `HeightNotFound`: No block at this height
    /// - `InvalidFinalization`: Height <= current finalized height
    fn mark_finalized(&mut self, height: u64) -> Result<(), StorageError>;

    /// Get the current storage metadata.
    fn get_metadata(&self) -> Result<StorageMetadata, StorageError>;

    /// Get the latest block height.
    fn get_latest_height(&self) -> Result<u64, StorageError>;

    /// Get the finalized block height.
    fn get_finalized_height(&self) -> Result<u64, StorageError>;

    /// Check if a block exists by hash.
    fn block_exists(&self, hash: &Hash) -> bool;

    /// Check if a block exists at height.
    fn block_exists_at_height(&self, height: u64) -> bool;

    /// V2.3: Get the location of a transaction by its hash.
    ///
    /// This API supports Transaction Indexing (Subsystem 3) for Merkle proof generation.
    ///
    /// ## Parameters
    ///
    /// - `transaction_hash`: Hash of the transaction to locate
    ///
    /// ## Returns
    ///
    /// - `Ok(TransactionLocation)`: Location data including block_hash, height, and index
    /// - `Err(TransactionNotFound)`: Transaction not in any stored block
    fn get_transaction_location(
        &self,
        transaction_hash: &Hash,
    ) -> Result<TransactionLocation, StorageError>;

    /// V2.3: Get ONLY the list of transaction hashes for a given block.
    ///
    /// This is a performance-optimized endpoint for the Transaction Indexing
    /// subsystem to use for rebuilding Merkle trees for proof generation.
    ///
    /// ## Parameters
    ///
    /// - `block_hash`: Hash of the block to get transaction hashes for
    ///
    /// ## Returns
    ///
    /// - `Ok(Vec<Hash>)`: Transaction hashes in canonical order
    /// - `Err(BlockNotFound)`: Block with this hash not found
    fn get_transaction_hashes_for_block(
        &self,
        block_hash: &Hash,
    ) -> Result<Vec<Hash>, StorageError>;
}

/// Event handler for the V2.3 Choreography pattern.
///
/// Block Storage is a Stateful Assembler that receives events from:
/// - Consensus (Subsystem 8): BlockValidated
/// - Transaction Indexing (Subsystem 3): MerkleRootComputed
/// - State Management (Subsystem 4): StateRootComputed
pub trait BlockAssemblerApi {
    /// Handle incoming BlockValidated event from Consensus.
    ///
    /// ## Authorization
    ///
    /// Only accepts events from Subsystem 8 (Consensus).
    fn on_block_validated(
        &mut self,
        sender_id: u8,
        block: ValidatedBlock,
        now: Timestamp,
    ) -> Result<(), StorageError>;

    /// Handle incoming MerkleRootComputed event from Transaction Indexing.
    ///
    /// ## Authorization
    ///
    /// Only accepts events from Subsystem 3 (Transaction Indexing).
    fn on_merkle_root_computed(
        &mut self,
        sender_id: u8,
        block_hash: Hash,
        merkle_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError>;

    /// Handle incoming StateRootComputed event from State Management.
    ///
    /// ## Authorization
    ///
    /// Only accepts events from Subsystem 4 (State Management).
    fn on_state_root_computed(
        &mut self,
        sender_id: u8,
        block_hash: Hash,
        state_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError>;

    /// Periodic garbage collection of expired assemblies (INVARIANT-7).
    ///
    /// Call at 5-second intervals from the runtime's GC task.
    /// Purges assemblies exceeding `assembly_timeout_secs` (default: 30s).
    ///
    /// Reference: SPEC-02 Section 2.6 (INVARIANT-7: Assembly Timeout)
    fn gc_expired_assemblies(&mut self, now: Timestamp) -> Vec<Hash>;

    /// Garbage collection returning full assembly data for event emission.
    ///
    /// Returns (block_hash, PendingBlockAssembly) tuples for `AssemblyTimeout` events.
    /// Used by the runtime to emit monitoring/alerting events.
    ///
    /// Reference: SPEC-02 Section 4.3 (AssemblyTimeoutPayload)
    fn gc_expired_assemblies_with_data(
        &mut self,
        now: Timestamp,
    ) -> Vec<(Hash, crate::domain::assembler::PendingBlockAssembly)>;
}
