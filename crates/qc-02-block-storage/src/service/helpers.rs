//! # Block Storage Service - Helper Methods
//!
//! Private helper methods for the BlockStorageService.

use super::*;

impl<KV, FS, CS, TS, BS> BlockStorageService<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    /// Load the block index from persistent storage.
    ///
    /// This rebuilds the in-memory index from the persisted height->hash mappings.
    pub(crate) fn load_index_from_storage(&mut self) -> Result<(), StorageError> {
        // Scan all height keys (prefix "h:")
        let height_prefix = KeyPrefix::BlockByHeight.as_bytes();
        let entries = self
            .kv_store
            .prefix_scan(height_prefix)
            .map_err(StorageError::from)?;

        if entries.is_empty() {
            #[cfg(feature = "tracing-log")]
            tracing::info!("[qc-02] No existing blocks found in storage");
            return Ok(());
        }

        let mut loaded_count = 0u64;
        let mut max_height = 0u64;

        for (key, value) in entries {
            // Key format: "h:" + 8-byte big-endian height
            if key.len() != 10 {
                continue; // Skip malformed keys
            }

            // Parse height from key
            let height_bytes: [u8; 8] =
                key[2..10]
                    .try_into()
                    .map_err(|_| StorageError::DatabaseError {
                        message: "Invalid height key format".to_string(),
                    })?;
            let height = u64::from_be_bytes(height_bytes);

            // Parse block hash from value
            if value.len() != 32 {
                continue; // Skip malformed values
            }
            let block_hash: Hash = value.try_into().map_err(|_| StorageError::DatabaseError {
                message: "Invalid block hash format".to_string(),
            })?;

            // Insert into in-memory index
            self.block_index.insert(height, block_hash);
            loaded_count += 1;
            max_height = max_height.max(height);
        }

        if loaded_count > 0 {
            #[cfg(feature = "tracing-log")]
            tracing::info!(
                "[qc-02] 💾 Loaded {} blocks from storage (height 0 to {})",
                loaded_count,
                max_height
            );

            // Update metadata with loaded state
            self.metadata.on_block_stored(
                max_height,
                self.block_index.get(max_height).unwrap_or([0u8; 32]),
            );
        }

        Ok(())
    }

    /// Check disk space (INVARIANT-2).
    pub(crate) fn check_disk_space(&self) -> Result<(), StorageError> {
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
    pub(crate) fn check_parent_exists(&self, block: &ValidatedBlock) -> Result<(), StorageError> {
        let height = block.header.height;

        // Genesis block has no parent requirement
        if height == 0 {
            return Ok(());
        }

        let parent_hash = block.header.parent_hash;
        if !self.block_exists_helper(&parent_hash) {
            return Err(StorageError::ParentNotFound { parent_hash });
        }
        Ok(())
    }

    /// Compute checksum for a stored block.
    pub(crate) fn compute_block_checksum(
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
    pub(crate) fn verify_block_checksum(&self, block: &StoredBlock) -> Result<(), StorageError> {
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
    pub(crate) fn compute_block_hash(&self, block: &ValidatedBlock) -> Hash {
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
    pub(crate) fn index_transactions(&mut self, block: &ValidatedBlock, block_hash: Hash) {
        for (index, tx) in block.transactions.iter().enumerate() {
            let location = TransactionLocation::new(
                block_hash,
                block.header.height,
                index,
                block.header.merkle_root,
            );

            // Always update in-memory index for fast lookups
            self.tx_index.insert(tx.tx_hash, location.clone());

            // Skip persistence if disabled
            if !self.config.persist_transaction_index {
                continue;
            }

            let key = KeyPrefix::transaction_key(&tx.tx_hash);
            let Ok(value) = bincode::serialize(&location) else {
                continue;
            };

            #[allow(unused_variables)]
            if let Err(e) = self.kv_store.put(&key, &value) {
                #[cfg(feature = "tracing-log")]
                tracing::warn!(
                    "[qc-02] Failed to persist tx index for {:x?}: {:?}",
                    &tx.tx_hash[..4],
                    e
                );
            }
        }
    }

    /// Load transaction index from persistent storage.
    pub(crate) fn load_transaction_index_from_storage(&mut self) -> Result<(), StorageError> {
        if !self.config.persist_transaction_index {
            return Ok(());
        }

        let tx_prefix = KeyPrefix::Transaction.as_bytes();
        let entries = self
            .kv_store
            .prefix_scan(tx_prefix)
            .map_err(StorageError::from)?;

        if entries.is_empty() {
            #[cfg(feature = "tracing-log")]
            tracing::info!("[qc-02] No existing transaction index found");
            return Ok(());
        }

        let mut loaded_count = 0u64;

        for (key, value) in entries {
            // Key format: "t:" + 32-byte tx_hash
            if key.len() != 34 {
                continue; // Skip malformed keys
            }

            let tx_hash: Hash = key[2..34]
                .try_into()
                .map_err(|_| StorageError::DatabaseError {
                    message: "Invalid tx hash format".to_string(),
                })?;

            let location: TransactionLocation =
                bincode::deserialize(&value).map_err(|e| StorageError::SerializationError {
                    message: format!("Failed to deserialize tx location: {}", e),
                })?;

            self.tx_index.insert(tx_hash, location);
            loaded_count += 1;
        }

        if loaded_count > 0 {
            #[cfg(feature = "tracing-log")]
            tracing::info!(
                "[qc-02] � Loaded {} transaction index entries from storage",
                loaded_count
            );
        }

        Ok(())
    }

    /// Try to complete an assembly and write the block.
    pub(crate) fn try_complete_assembly(
        &mut self,
        block_hash: Hash,
    ) -> Result<Option<Hash>, StorageError> {
        if let Some(assembly) = self.assembly_buffer.take_complete(&block_hash) {
            if let Some((block, merkle_root, state_root)) = assembly.take_components() {
                // All components present - write the block
                let hash = self.write_block(block, merkle_root, state_root)?;
                return Ok(Some(hash));
            }
        }
        Ok(None)
    }

    /// Check if a block exists by hash  (helper method for internal use).
    pub(crate) fn block_exists_helper(&self, hash: &Hash) -> bool {
        let key = KeyPrefix::block_key(hash);
        matches!(self.kv_store.get(&key), Ok(Some(_)))
    }
}
