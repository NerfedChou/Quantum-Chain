//! # Block Storage API Implementation
//!
//! Implements the BlockStorageApi trait for read/write operations.

use super::*;
use crate::ports::inbound::BlockStorageApi;

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
        let height = block.header.height;
        #[cfg(feature = "tracing-log")]
        tracing::info!("[qc-02] 📦 Writing block #{} to storage", height);

        // INVARIANT-2: Check disk space
        self.check_disk_space()?;

        // INVARIANT-1: Check parent exists
        self.check_parent_exists(&block)?;

        let block_hash = self.compute_block_hash(&block);

        // Security: Verify hash is valid (non-zero)
        verify_block_hash_nonzero(&block_hash)?;

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

        #[cfg(feature = "tracing-log")]
        tracing::info!(
            "[qc-02] ✓ Block #{} stored! Hash: 0x{}, Txs: {}",
            height,
            hex::encode(&block_hash[..8]),
            block.transactions.len()
        );

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
