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

use crate::domain::assembler::BlockAssemblyBuffer;
use crate::domain::entities::{BlockIndex, StorageMetadata, StoredBlock, Timestamp};
use crate::domain::errors::StorageError;
use crate::domain::value_objects::{KeyPrefix, StorageConfig, TransactionLocation};
use crate::ports::inbound::{BlockAssemblerApi, BlockStorageApi};
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
/// Implements both `BlockStorageApi` and `BlockAssemblerApi`.
pub struct BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    /// Key-value store for persistence.
    kv_store: KV,
    /// Filesystem adapter for disk space checks.
    fs_adapter: FS,
    /// Checksum provider for data integrity.
    checksum: CS,
    /// Time source for timestamps.
    time_source: TS,
    /// Block serializer.
    serializer: BS,
    /// Service configuration.
    config: StorageConfig,
    /// Assembly buffer for V2.3 choreography.
    assembly_buffer: BlockAssemblyBuffer,
    /// In-memory block index (height -> hash).
    block_index: BlockIndex,
    /// In-memory metadata.
    metadata: StorageMetadata,
    /// Transaction index (tx_hash -> location).
    tx_index: HashMap<Hash, TransactionLocation>,
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
    pub fn new(
        kv_store: KV,
        fs_adapter: FS,
        checksum: CS,
        time_source: TS,
        serializer: BS,
        config: StorageConfig,
    ) -> Self {
        let assembly_buffer = BlockAssemblyBuffer::new(config.assembly_config.clone());

        Self {
            kv_store,
            fs_adapter,
            checksum,
            time_source,
            serializer,
            config,
            assembly_buffer,
            block_index: BlockIndex::new(),
            metadata: StorageMetadata::default(),
            tx_index: HashMap::new(),
        }
    }

    /// Check disk space (INVARIANT-2).
    fn check_disk_space(&self) -> Result<(), StorageError> {
        let available = self
            .fs_adapter
            .available_disk_space_percent()
            .map_err(|e| StorageError::DatabaseError {
                message: format!("Failed to check disk space: {:?}", e),
            })?;

        if available < self.config.min_disk_space_percent {
            return Err(StorageError::DiskFull {
                available_percent: available,
                required_percent: self.config.min_disk_space_percent,
            });
        }
        Ok(())
    }

    /// Check parent exists (INVARIANT-1).
    fn check_parent_exists(&self, block: &ValidatedBlock) -> Result<(), StorageError> {
        let height = block.header.height;

        // Genesis block has no parent requirement
        if height == 0 {
            return Ok(());
        }

        let parent_hash = block.header.parent_hash;
        if !self.block_exists(&parent_hash) {
            return Err(StorageError::ParentNotFound { parent_hash });
        }
        Ok(())
    }

    /// Compute checksum for a stored block.
    fn compute_block_checksum(
        &self,
        block: &ValidatedBlock,
        merkle_root: &Hash,
        state_root: &Hash,
    ) -> u32 {
        let mut data = Vec::new();
        data.extend_from_slice(&block.header.parent_hash);
        data.extend_from_slice(&block.header.height.to_le_bytes());
        data.extend_from_slice(merkle_root);
        data.extend_from_slice(state_root);
        self.checksum.compute_crc32c(&data)
    }

    /// Verify checksum on read (INVARIANT-3).
    ///
    /// Note: Checksums are ALWAYS verified - this is a compile-time guarantee.
    fn verify_block_checksum(&self, block: &StoredBlock) -> Result<(), StorageError> {
        // SECURITY: verify_checksums() always returns true (compile-time guarantee)
        // This check is kept for clarity but the method is const fn returning true
        debug_assert!(self.config.verify_checksums());

        let expected =
            self.compute_block_checksum(&block.block, &block.merkle_root, &block.state_root);

        if block.checksum != expected {
            let block_hash = block.block_hash();
            return Err(StorageError::DataCorruption {
                block_hash,
                expected_checksum: expected,
                actual_checksum: block.checksum,
            });
        }
        Ok(())
    }

    /// Compute block hash from header.
    fn compute_block_hash(&self, block: &ValidatedBlock) -> Hash {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(block.header.parent_hash);
        hasher.update(block.header.height.to_le_bytes());
        hasher.update(block.header.merkle_root);
        hasher.update(block.header.state_root);
        hasher.update(block.header.timestamp.to_le_bytes());
        hasher.finalize().into()
    }

    /// Index transactions in a block.
    fn index_transactions(&mut self, block: &ValidatedBlock, block_hash: Hash) {
        for (index, tx) in block.transactions.iter().enumerate() {
            let location = TransactionLocation::new(
                block_hash,
                block.header.height,
                index,
                block.header.merkle_root,
            );
            self.tx_index.insert(tx.tx_hash, location);
        }
    }

    /// Try to complete an assembly and write the block.
    fn try_complete_assembly(&mut self, block_hash: Hash) -> Result<Option<Hash>, StorageError> {
        if let Some(assembly) = self.assembly_buffer.take_complete(&block_hash) {
            if let Some((block, merkle_root, state_root)) = assembly.take_components() {
                // All components present - write the block
                let hash = self.write_block(block, merkle_root, state_root)?;
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }
}

impl<KV, FS, CS, TS, BS> BlockStorageApi for BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    fn write_block(
        &mut self,
        block: ValidatedBlock,
        merkle_root: Hash,
        state_root: Hash,
    ) -> Result<Hash, StorageError> {
        // INVARIANT-2: Check disk space
        self.check_disk_space()?;

        // INVARIANT-1: Check parent exists
        self.check_parent_exists(&block)?;

        let block_hash = self.compute_block_hash(&block);
        let height = block.header.height;

        // Check block doesn't already exist
        if self.block_exists(&block_hash) {
            return Err(StorageError::BlockExists { hash: block_hash });
        }

        // Compute checksum
        let checksum = self.compute_block_checksum(&block, &merkle_root, &state_root);
        let now = self.time_source.now();

        // Create stored block
        let stored_block = StoredBlock::new(block.clone(), merkle_root, state_root, now, checksum);

        // Check block size
        let size = self.serializer.estimate_size(&stored_block);
        if size > self.config.max_block_size {
            return Err(StorageError::BlockTooLarge {
                size,
                max_size: self.config.max_block_size,
            });
        }

        // Serialize
        let data = self
            .serializer
            .serialize(&stored_block)
            .map_err(StorageError::from)?;

        // INVARIANT-4: Atomic batch write
        let operations = vec![
            BatchOperation::put(KeyPrefix::block_key(&block_hash), data),
            BatchOperation::put(KeyPrefix::height_key(height), block_hash.to_vec()),
        ];

        self.kv_store
            .atomic_batch_write(operations)
            .map_err(StorageError::from)?;

        // Update in-memory state
        self.block_index.insert(height, block_hash);
        self.metadata.on_block_stored(height, block_hash);
        self.index_transactions(&block, block_hash);

        Ok(block_hash)
    }

    fn read_block(&self, hash: &Hash) -> Result<StoredBlock, StorageError> {
        let key = KeyPrefix::block_key(hash);

        let data = self
            .kv_store
            .get(&key)
            .map_err(StorageError::from)?
            .ok_or(StorageError::BlockNotFound { hash: *hash })?;

        let block = self
            .serializer
            .deserialize(&data)
            .map_err(StorageError::from)?;

        // INVARIANT-3: Verify checksum
        self.verify_block_checksum(&block)?;

        Ok(block)
    }

    fn read_block_by_height(&self, height: u64) -> Result<StoredBlock, StorageError> {
        let hash = self
            .block_index
            .get(height)
            .ok_or(StorageError::HeightNotFound { height })?;

        self.read_block(&hash)
    }

    fn read_block_range(
        &self,
        start_height: u64,
        limit: u64,
    ) -> Result<Vec<StoredBlock>, StorageError> {
        // Cap limit at 100
        let limit = limit.min(100);

        // Check start height exists
        if !self.block_index.contains(start_height) {
            return Err(StorageError::HeightNotFound {
                height: start_height,
            });
        }

        let mut blocks = Vec::with_capacity(limit as usize);

        for height in start_height..(start_height + limit) {
            match self.read_block_by_height(height) {
                Ok(block) => blocks.push(block),
                Err(StorageError::HeightNotFound { .. }) => break, // End of chain
                Err(e) => return Err(e),
            }
        }

        Ok(blocks)
    }

    fn mark_finalized(&mut self, height: u64) -> Result<(), StorageError> {
        // Check block exists
        if !self.block_index.contains(height) {
            return Err(StorageError::HeightNotFound { height });
        }

        // INVARIANT-5: Finalization monotonicity
        if !self.metadata.on_finalized(height) {
            return Err(StorageError::InvalidFinalization {
                requested: height,
                current: self.metadata.finalized_height,
            });
        }

        Ok(())
    }

    fn get_metadata(&self) -> Result<StorageMetadata, StorageError> {
        Ok(self.metadata.clone())
    }

    fn get_latest_height(&self) -> Result<u64, StorageError> {
        Ok(self.metadata.latest_height)
    }

    fn get_finalized_height(&self) -> Result<u64, StorageError> {
        Ok(self.metadata.finalized_height)
    }

    fn block_exists(&self, hash: &Hash) -> bool {
        self.kv_store
            .exists(&KeyPrefix::block_key(hash))
            .unwrap_or(false)
    }

    fn block_exists_at_height(&self, height: u64) -> bool {
        self.block_index.contains(height)
    }

    fn get_transaction_location(
        &self,
        transaction_hash: &Hash,
    ) -> Result<TransactionLocation, StorageError> {
        self.tx_index
            .get(transaction_hash)
            .cloned()
            .ok_or(StorageError::TransactionNotFound {
                tx_hash: *transaction_hash,
            })
    }

    fn get_transaction_hashes_for_block(
        &self,
        block_hash: &Hash,
    ) -> Result<Vec<Hash>, StorageError> {
        let block = self.read_block(block_hash)?;
        let hashes: Vec<Hash> = block
            .block
            .transactions
            .iter()
            .map(|tx| tx.tx_hash)
            .collect();
        Ok(hashes)
    }
}

impl<KV, FS, CS, TS, BS> BlockAssemblerApi for BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    fn on_block_validated(
        &mut self,
        sender_id: u8,
        block: ValidatedBlock,
        now: Timestamp,
    ) -> Result<(), StorageError> {
        // Authorization: Only Consensus (Subsystem 8)
        if sender_id != subsystem_ids::CONSENSUS {
            return Err(StorageError::UnauthorizedSender {
                sender_id,
                expected_id: subsystem_ids::CONSENSUS,
                operation: "BlockValidated",
            });
        }

        let block_hash = self.compute_block_hash(&block);
        self.assembly_buffer
            .add_block_validated(block_hash, block, now);

        // Enforce buffer limit (INVARIANT-8)
        let purged = self.assembly_buffer.enforce_max_pending();
        for hash in purged {
            // Log purged assemblies
            eprintln!(
                "WARNING: Assembly buffer full, purged block {:?}",
                &hash[..4]
            );
        }

        // Try to complete
        self.try_complete_assembly(block_hash)?;

        Ok(())
    }

    fn on_merkle_root_computed(
        &mut self,
        sender_id: u8,
        block_hash: Hash,
        merkle_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError> {
        // Authorization: Only Transaction Indexing (Subsystem 3)
        if sender_id != subsystem_ids::TRANSACTION_INDEXING {
            return Err(StorageError::UnauthorizedSender {
                sender_id,
                expected_id: subsystem_ids::TRANSACTION_INDEXING,
                operation: "MerkleRootComputed",
            });
        }

        self.assembly_buffer
            .add_merkle_root(block_hash, merkle_root, now);

        // Try to complete
        self.try_complete_assembly(block_hash)?;

        Ok(())
    }

    fn on_state_root_computed(
        &mut self,
        sender_id: u8,
        block_hash: Hash,
        state_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError> {
        // Authorization: Only State Management (Subsystem 4)
        if sender_id != subsystem_ids::STATE_MANAGEMENT {
            return Err(StorageError::UnauthorizedSender {
                sender_id,
                expected_id: subsystem_ids::STATE_MANAGEMENT,
                operation: "StateRootComputed",
            });
        }

        self.assembly_buffer
            .add_state_root(block_hash, state_root, now);

        // Try to complete
        self.try_complete_assembly(block_hash)?;

        Ok(())
    }

    fn gc_expired_assemblies(&mut self, now: Timestamp) -> Vec<Hash> {
        self.assembly_buffer.gc_expired(now)
    }

    fn gc_expired_assemblies_with_data(
        &mut self,
        now: Timestamp,
    ) -> Vec<(Hash, crate::domain::assembler::PendingBlockAssembly)> {
        self.assembly_buffer.gc_expired_with_data(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::outbound::{
        BincodeBlockSerializer, DefaultChecksumProvider, InMemoryKVStore, MockFileSystemAdapter,
        SystemTimeSource,
    };
    use shared_types::{BlockHeader, ConsensusProof};

    fn make_test_service() -> BlockStorageService<
        InMemoryKVStore,
        MockFileSystemAdapter,
        DefaultChecksumProvider,
        SystemTimeSource,
        BincodeBlockSerializer,
    > {
        BlockStorageService::new(
            InMemoryKVStore::new(),
            MockFileSystemAdapter::new(50),
            DefaultChecksumProvider,
            SystemTimeSource,
            BincodeBlockSerializer,
            StorageConfig::default(),
        )
    }

    fn make_test_block(height: u64, parent_hash: Hash) -> ValidatedBlock {
        ValidatedBlock {
            header: BlockHeader {
                version: 1,
                height,
                parent_hash,
                merkle_root: [0; 32],
                state_root: [0; 32],
                timestamp: 1000,
                proposer: [0; 32],
            },
            transactions: vec![],
            consensus_proof: ConsensusProof::default(),
        }
    }

    #[test]
    fn test_write_and_read_block() {
        let mut service = make_test_service();

        let block = make_test_block(0, [0; 32]);
        let merkle_root = [0xAA; 32];
        let state_root = [0xBB; 32];

        let hash = service
            .write_block(block.clone(), merkle_root, state_root)
            .unwrap();

        let stored = service.read_block(&hash).unwrap();
        assert_eq!(stored.merkle_root, merkle_root);
        assert_eq!(stored.state_root, state_root);
        assert_eq!(stored.block.header.height, 0);
    }

    #[test]
    fn test_disk_full_invariant() {
        let mut service = BlockStorageService::new(
            InMemoryKVStore::new(),
            MockFileSystemAdapter::new(4), // Below 5% threshold
            DefaultChecksumProvider,
            SystemTimeSource,
            BincodeBlockSerializer,
            StorageConfig::default(),
        );

        let block = make_test_block(0, [0; 32]);
        let result = service.write_block(block, [0; 32], [0; 32]);

        assert!(matches!(result, Err(StorageError::DiskFull { .. })));
    }

    #[test]
    fn test_parent_not_found_invariant() {
        let mut service = make_test_service();

        // Try to write block at height 5 without parents
        let block = make_test_block(5, [0xFF; 32]);
        let result = service.write_block(block, [0; 32], [0; 32]);

        assert!(matches!(result, Err(StorageError::ParentNotFound { .. })));
    }

    #[test]
    fn test_finalization_monotonicity() {
        let mut service = make_test_service();

        // Write 10 blocks
        let mut parent_hash = [0; 32];
        for height in 0..10 {
            let block = make_test_block(height, parent_hash);
            parent_hash = service.write_block(block, [0; 32], [0; 32]).unwrap();
        }

        // Finalize height 5
        service.mark_finalized(5).unwrap();

        // Cannot regress to height 3
        let result = service.mark_finalized(3);
        assert!(matches!(
            result,
            Err(StorageError::InvalidFinalization { .. })
        ));

        // Can finalize higher
        service.mark_finalized(7).unwrap();
        assert_eq!(service.get_finalized_height().unwrap(), 7);
    }

    #[test]
    fn test_choreography_assembly() {
        let mut service = make_test_service();

        let block = make_test_block(0, [0; 32]);
        let block_hash = service.compute_block_hash(&block);
        let now = 1000;

        // Send events in choreography order
        service
            .on_block_validated(subsystem_ids::CONSENSUS, block.clone(), now)
            .unwrap();

        // Block not written yet (need merkle + state)
        assert!(!service.block_exists(&block_hash));

        service
            .on_merkle_root_computed(
                subsystem_ids::TRANSACTION_INDEXING,
                block_hash,
                [0xAA; 32],
                now,
            )
            .unwrap();

        // Still not written
        assert!(!service.block_exists(&block_hash));

        service
            .on_state_root_computed(subsystem_ids::STATE_MANAGEMENT, block_hash, [0xBB; 32], now)
            .unwrap();

        // INVARIANT-4: All 3 components arrived → atomic write completed
        // Verify via height (service recomputes block_hash internally)
        assert!(service.block_exists_at_height(0));
    }

    #[test]
    fn test_unauthorized_sender_rejected() {
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);
        let now = 1000;

        // Try sending BlockValidated from wrong subsystem
        let result = service.on_block_validated(subsystem_ids::TRANSACTION_INDEXING, block, now);
        assert!(matches!(
            result,
            Err(StorageError::UnauthorizedSender { .. })
        ));
    }

    // =========================================================================
    // TEST GROUP 1: Atomic Write Guarantees (SPEC-02 Section 5.1)
    // =========================================================================

    #[test]
    fn test_write_includes_all_required_entries() {
        // Verify: Block, height index, merkle root, state root all written
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);
        let merkle_root = [0xAA; 32];
        let state_root = [0xBB; 32];

        let hash = service.write_block(block, merkle_root, state_root).unwrap();

        // Verify block exists by hash
        assert!(service.block_exists(&hash));
        // Verify block exists by height
        assert!(service.block_exists_at_height(0));
        // Verify we can read the block
        let stored = service.read_block(&hash).unwrap();
        assert_eq!(stored.merkle_root, merkle_root);
        assert_eq!(stored.state_root, state_root);
    }

    // =========================================================================
    // TEST GROUP 2: Disk Space Safety
    // Reference: SPEC-02 Section 5.1 (INVARIANT-2 Tests)
    // =========================================================================

    #[test]
    fn test_write_succeeds_when_disk_at_5_percent() {
        // INVARIANT-2 boundary: exactly 5% passes threshold check
        let mut service = BlockStorageService::new(
            InMemoryKVStore::new(),
            MockFileSystemAdapter::new(5),
            DefaultChecksumProvider,
            SystemTimeSource,
            BincodeBlockSerializer,
            StorageConfig::default(),
        );

        let block = make_test_block(0, [0; 32]);
        let result = service.write_block(block, [0; 32], [0; 32]);

        assert!(result.is_ok());
    }

    // =========================================================================
    // TEST GROUP 3: Data Integrity / Checksum
    // Reference: SPEC-02 Section 5.1 (INVARIANT-3 Tests)
    // =========================================================================

    #[test]
    fn test_valid_checksum_passes_verification() {
        // INVARIANT-3: Checksum computed at write, verified at read
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);

        let hash = service.write_block(block, [0xAA; 32], [0xBB; 32]).unwrap();

        // Checksum verification is automatic in read_block()
        let result = service.read_block(&hash);
        assert!(result.is_ok());
    }

    // =========================================================================
    // TEST GROUP 4: Sequential Block Requirement
    // Reference: SPEC-02 Section 5.1 (INVARIANT-1 Tests)
    // =========================================================================

    #[test]
    fn test_genesis_block_has_no_parent_requirement() {
        // INVARIANT-1 exception: Genesis block (height=0) has no parent
        let mut service = make_test_service();
        let genesis = make_test_block(0, [0; 32]);

        let result = service.write_block(genesis, [0; 32], [0; 32]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_succeeds_with_parent_present() {
        // Verify: INVARIANT-1 happy path
        let mut service = make_test_service();

        // Write genesis
        let genesis = make_test_block(0, [0; 32]);
        let genesis_hash = service.write_block(genesis, [0; 32], [0; 32]).unwrap();

        // Write child block with correct parent
        let child = make_test_block(1, genesis_hash);
        let result = service.write_block(child, [0; 32], [0; 32]);

        assert!(result.is_ok());
    }

    // =========================================================================
    // TEST GROUP 5: Finalization Logic (SPEC-02 Section 5.1)
    // =========================================================================

    #[test]
    fn test_finalization_rejects_same_height() {
        // Verify: INVARIANT-5 - cannot finalize same height twice
        let mut service = make_test_service();

        // Write blocks 0-5
        let mut parent_hash = [0; 32];
        for height in 0..6 {
            let block = make_test_block(height, parent_hash);
            parent_hash = service.write_block(block, [0; 32], [0; 32]).unwrap();
        }

        // Finalize height 3
        service.mark_finalized(3).unwrap();

        // Try to finalize height 3 again
        let result = service.mark_finalized(3);
        assert!(matches!(
            result,
            Err(StorageError::InvalidFinalization { .. })
        ));
    }

    #[test]
    fn test_finalization_requires_block_exists() {
        // Verify: Cannot finalize non-existent block
        let mut service = make_test_service();

        // Write only genesis
        let genesis = make_test_block(0, [0; 32]);
        service.write_block(genesis, [0; 32], [0; 32]).unwrap();

        // Try to finalize height 100 (doesn't exist)
        let result = service.mark_finalized(100);
        assert!(matches!(result, Err(StorageError::HeightNotFound { .. })));
    }

    // =========================================================================
    // TEST GROUP 6: Access Control
    // Reference: SPEC-02 Section 5.1 / IPC-MATRIX.md (Sender Authorization)
    // =========================================================================

    #[test]
    fn test_merkle_root_rejects_wrong_sender() {
        // IPC-MATRIX: MerkleRootComputed MUST come from TRANSACTION_INDEXING (3)
        let mut service = make_test_service();
        let block_hash = [0xAB; 32];
        let now = 1000;

        // Sender CONSENSUS (8) violates IPC-MATRIX authorization
        let result =
            service.on_merkle_root_computed(subsystem_ids::CONSENSUS, block_hash, [0xAA; 32], now);
        assert!(matches!(
            result,
            Err(StorageError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_state_root_rejects_wrong_sender() {
        // IPC-MATRIX: StateRootComputed MUST come from STATE_MANAGEMENT (4)
        let mut service = make_test_service();
        let block_hash = [0xAB; 32];
        let now = 1000;

        // Sender CONSENSUS (8) violates IPC-MATRIX authorization
        let result =
            service.on_state_root_computed(subsystem_ids::CONSENSUS, block_hash, [0xBB; 32], now);
        assert!(matches!(
            result,
            Err(StorageError::UnauthorizedSender { .. })
        ));
    }

    #[test]
    fn test_block_validated_accepts_only_consensus() {
        // IPC-MATRIX: BlockValidated MUST come from CONSENSUS (8)
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);
        let now = 1000;

        // Sender STATE_MANAGEMENT (4) violates IPC-MATRIX authorization
        let result = service.on_block_validated(subsystem_ids::STATE_MANAGEMENT, block, now);
        assert!(matches!(
            result,
            Err(StorageError::UnauthorizedSender { .. })
        ));
    }

    // =========================================================================
    // TEST GROUP 7: Batch Read / Node Syncing
    // Reference: SPEC-02 Section 5.1 (read_block_range Tests)
    // =========================================================================

    #[test]
    fn test_read_block_range_returns_sequential_blocks() {
        // Batch read returns blocks in ascending height order
        let mut service = make_test_service();

        let mut parent_hash = [0; 32];
        for height in 0..21 {
            let block = make_test_block(height, parent_hash);
            parent_hash = service.write_block(block, [0; 32], [0; 32]).unwrap();
        }

        // Read range 5-14 (10 blocks)
        let blocks = service.read_block_range(5, 10).unwrap();

        assert_eq!(blocks.len(), 10);
        for (i, block) in blocks.iter().enumerate() {
            assert_eq!(block.block.header.height, 5 + i as u64);
        }
    }

    #[test]
    fn test_read_block_range_respects_limit_cap() {
        // API constraint: limit capped at 100 to prevent resource exhaustion
        let mut service = make_test_service();

        let mut parent_hash = [0; 32];
        for height in 0..150 {
            let block = make_test_block(height, parent_hash);
            parent_hash = service.write_block(block, [0; 32], [0; 32]).unwrap();
        }

        // Request 500 → capped to 100
        let blocks = service.read_block_range(0, 500).unwrap();

        assert_eq!(blocks.len(), 100);
    }

    #[test]
    fn test_read_block_range_returns_partial_if_chain_end() {
        // Returns available blocks when chain is shorter than requested
        let mut service = make_test_service();

        let mut parent_hash = [0; 32];
        for height in 0..10 {
            let block = make_test_block(height, parent_hash);
            parent_hash = service.write_block(block, [0; 32], [0; 32]).unwrap();
        }

        // Request 20 blocks starting at height 5 → returns 5 (heights 5-9)
        let blocks = service.read_block_range(5, 20).unwrap();

        assert_eq!(blocks.len(), 5);
    }

    #[test]
    fn test_read_block_range_fails_on_invalid_start() {
        // HeightNotFound error when start_height doesn't exist
        let mut service = make_test_service();

        let genesis = make_test_block(0, [0; 32]);
        service.write_block(genesis, [0; 32], [0; 32]).unwrap();

        let result = service.read_block_range(100, 10);
        assert!(matches!(result, Err(StorageError::HeightNotFound { .. })));
    }

    // =========================================================================
    // TEST GROUP 10: Stateful Assembler (SPEC-02 Section 5.1)
    // =========================================================================

    #[test]
    fn test_assembly_buffers_partial_components() {
        // Choreography: buffered until all 3 components arrive
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);
        let block_hash = service.compute_block_hash(&block);
        let now = 1000;

        // Only 2 of 3 components arrived
        service
            .on_block_validated(subsystem_ids::CONSENSUS, block, now)
            .unwrap();
        service
            .on_merkle_root_computed(
                subsystem_ids::TRANSACTION_INDEXING,
                block_hash,
                [0xAA; 32],
                now,
            )
            .unwrap();

        // Incomplete assembly: no write yet
        assert!(!service.block_exists_at_height(0));
    }

    #[test]
    fn test_assembly_works_state_first() {
        // Choreography: order-independent assembly
        let mut service = make_test_service();
        let block = make_test_block(0, [0; 32]);
        let block_hash = service.compute_block_hash(&block);
        let now = 1000;

        // Reverse order: StateRoot → MerkleRoot → BlockValidated
        service
            .on_state_root_computed(subsystem_ids::STATE_MANAGEMENT, block_hash, [0xBB; 32], now)
            .unwrap();

        service
            .on_merkle_root_computed(
                subsystem_ids::TRANSACTION_INDEXING,
                block_hash,
                [0xAA; 32],
                now,
            )
            .unwrap();

        service
            .on_block_validated(subsystem_ids::CONSENSUS, block, now)
            .unwrap();

        // All 3 arrived: atomic write completed
        assert!(service.block_exists_at_height(0));
    }

    // =========================================================================
    // TEST GROUP 11: Transaction Data Retrieval V2.3
    // Reference: SPEC-02 Section 5.1 (Transaction Lookup Contract)
    // =========================================================================

    #[test]
    fn test_get_transaction_location_returns_not_found() {
        // TransactionNotFound error for unknown transaction hash
        let service = make_test_service();
        let unknown_tx_hash = [0xFF; 32];

        let result = service.get_transaction_location(&unknown_tx_hash);
        assert!(matches!(
            result,
            Err(StorageError::TransactionNotFound { .. })
        ));
    }

    #[test]
    fn test_get_transaction_hashes_for_block_not_found() {
        // Verify: Block not found error
        let service = make_test_service();
        let unknown_block_hash = [0xFF; 32];

        let result = service.get_transaction_hashes_for_block(&unknown_block_hash);
        assert!(matches!(result, Err(StorageError::BlockNotFound { .. })));
    }

    #[test]
    fn test_block_exists_returns_false_for_unknown() {
        // Verify: block_exists correctly returns false
        let service = make_test_service();
        let unknown_hash = [0xFF; 32];

        assert!(!service.block_exists(&unknown_hash));
    }

    #[test]
    fn test_block_exists_at_height_returns_false_for_unknown() {
        // Verify: block_exists_at_height correctly returns false
        let service = make_test_service();

        assert!(!service.block_exists_at_height(999));
    }

    #[test]
    fn test_get_metadata_returns_default_on_empty() {
        // Verify: Metadata is valid on empty storage
        let service = make_test_service();

        let metadata = service.get_metadata().unwrap();
        assert_eq!(metadata.latest_height, 0);
        assert_eq!(metadata.finalized_height, 0);
    }

    #[test]
    fn test_get_latest_height_updates_after_write() {
        // Verify: Latest height updates after block write
        let mut service = make_test_service();

        // Initially 0
        assert_eq!(service.get_latest_height().unwrap(), 0);

        // Write genesis
        let genesis = make_test_block(0, [0; 32]);
        let genesis_hash = service.write_block(genesis, [0; 32], [0; 32]).unwrap();

        // Write block 1
        let block1 = make_test_block(1, genesis_hash);
        service.write_block(block1, [0; 32], [0; 32]).unwrap();

        assert_eq!(service.get_latest_height().unwrap(), 1);
    }

    #[test]
    fn test_duplicate_block_write_rejected() {
        // Verify: Cannot write same block twice
        let mut service = make_test_service();

        let block = make_test_block(0, [0; 32]);

        // First write succeeds
        let hash = service
            .write_block(block.clone(), [0xAA; 32], [0xBB; 32])
            .unwrap();

        // Second write with same block should fail
        let result = service.write_block(block, [0xAA; 32], [0xBB; 32]);
        assert!(matches!(result, Err(StorageError::BlockExists { .. })));
    }
}
