//! # Block Assembler API Implementation
//!
//! Implements the BlockAssemblerApi trait for V2.3 choreography.

use super::*;
use crate::domain::assembler::PendingBlockAssembly;
use crate::ports::inbound::BlockAssemblerApi;

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
        block: ValidatedBlock,
        now: Timestamp,
    ) -> Result<(), StorageError> {
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
        block_hash: Hash,
        merkle_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError> {
        self.assembly_buffer
            .add_merkle_root(block_hash, merkle_root, now);

        // Try to complete
        self.try_complete_assembly(block_hash)?;

        Ok(())
    }

    fn on_state_root_computed(
        &mut self,
        block_hash: Hash,
        state_root: Hash,
        now: Timestamp,
    ) -> Result<(), StorageError> {
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
    ) -> Vec<(Hash, PendingBlockAssembly)> {
        self.assembly_buffer.gc_expired_with_data(now)
    }
}
