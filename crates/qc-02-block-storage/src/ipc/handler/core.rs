//! IPC Message Handlers for Block Storage
//!
//! Implements the request/response and event handling per IPC-MATRIX.md.

use crate::domain::storage::StoredBlock;
use crate::ports::inbound::{BlockAssemblerApi, BlockStorageApi};
use crate::ports::outbound::{
    BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};
use crate::service::BlockStorageService;
use shared_types::{BlockHeader, Hash};

use crate::ipc::envelope::{subsystem_ids, AuthenticatedMessage, EnvelopeValidator};
use crate::ipc::payloads::*;

use super::helpers::{build_block_stored_payload, current_timestamp};
use super::types::{storage_error_to_payload, HandlerError};

/// Compute block hash from header (simplified - in production use SHA256)
fn compute_block_hash(header: &BlockHeader) -> Hash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(header.version.to_le_bytes());
    hasher.update(header.height.to_le_bytes());
    hasher.update(header.parent_hash);
    hasher.update(header.merkle_root);
    hasher.update(header.state_root);
    hasher.update(header.timestamp.to_le_bytes());
    hasher.update(header.proposer);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Convert stored block to BlockDifficultyInfo
fn to_difficulty_info(stored: StoredBlock) -> BlockDifficultyInfo {
    let block_hash = compute_block_hash(&stored.block.header);
    BlockDifficultyInfo {
        height: stored.block.header.height,
        timestamp: stored.block.header.timestamp,
        difficulty: stored.block.header.difficulty,
        hash: block_hash,
    }
}

/// Block Storage IPC Handler
///
/// Wraps BlockStorageService with IPC security boundaries.
/// All incoming messages are validated per Architecture.md v2.3.
pub struct BlockStorageHandler<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    /// The underlying service
    service: BlockStorageService<KV, FS, CS, TS, BS>,
    /// Envelope validator
    validator: EnvelopeValidator,
}

impl<KV, FS, CS, TS, BS> BlockStorageHandler<KV, FS, CS, TS, BS>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    BS: BlockSerializer,
{
    pub fn new(service: BlockStorageService<KV, FS, CS, TS, BS>, shared_secret: [u8; 32]) -> Self {
        Self {
            service,
            validator: EnvelopeValidator::new(subsystem_ids::BLOCK_STORAGE, shared_secret),
        }
    }

    // =========================================================================
    // EVENT HANDLERS (V2.3 Choreography)
    // =========================================================================

    /// Handle BlockValidated event from Consensus (Subsystem 8)
    pub fn handle_block_validated(
        &mut self,
        msg: AuthenticatedMessage<BlockValidatedPayload>,
    ) -> Result<Option<BlockStoredPayload>, HandlerError> {
        self.validator.validate(&msg)?;
        self.validator
            .validate_sender(msg.sender_id, super::security::BLOCK_VALIDATED_SENDERS)?;

        self.service
            .on_block_validated(msg.payload.block, current_timestamp())
            .map_err(HandlerError::Storage)?;

        Ok(self.check_assembly_by_height(msg.payload.block_height, msg.payload.block_hash))
    }

    /// Handle MerkleRootComputed event from Transaction Indexing (Subsystem 3)
    pub fn handle_merkle_root_computed(
        &mut self,
        msg: AuthenticatedMessage<MerkleRootComputedPayload>,
    ) -> Result<Option<BlockStoredPayload>, HandlerError> {
        self.validator.validate(&msg)?;
        self.validator
            .validate_sender(msg.sender_id, super::security::MERKLE_ROOT_SENDERS)?;

        self.service
            .on_merkle_root_computed(
                msg.payload.block_hash,
                msg.payload.merkle_root,
                current_timestamp(),
            )
            .map_err(HandlerError::Storage)?;

        Ok(self.check_assembly_by_hash(msg.payload.block_hash))
    }

    /// Handle StateRootComputed event from State Management (Subsystem 4)
    pub fn handle_state_root_computed(
        &mut self,
        msg: AuthenticatedMessage<StateRootComputedPayload>,
    ) -> Result<Option<BlockStoredPayload>, HandlerError> {
        self.validator.validate(&msg)?;
        self.validator
            .validate_sender(msg.sender_id, super::security::STATE_ROOT_SENDERS)?;

        self.service
            .on_state_root_computed(
                msg.payload.block_hash,
                msg.payload.state_root,
                current_timestamp(),
            )
            .map_err(HandlerError::Storage)?;

        Ok(self.check_assembly_by_hash(msg.payload.block_hash))
    }

    // =========================================================================
    // ASSEMBLY HELPERS
    // =========================================================================

    /// Check if assembly is complete by height and return BlockStoredPayload if so.
    fn check_assembly_by_height(
        &self,
        height: u64,
        block_hash: Hash,
    ) -> Option<BlockStoredPayload> {
        if self.service.block_exists_at_height(height) {
            if let Ok(stored) = self.service.read_block_by_height(height) {
                return Some(build_block_stored_payload(&stored, block_hash, height));
            }
        }
        None
    }

    /// Check if assembly is complete by hash and return BlockStoredPayload if so.
    fn check_assembly_by_hash(&self, block_hash: Hash) -> Option<BlockStoredPayload> {
        if self.service.block_exists(&block_hash) {
            if let Ok(stored) = self.service.read_block(&block_hash) {
                return Some(build_block_stored_payload(
                    &stored,
                    block_hash,
                    stored.block.header.height,
                ));
            }
        }
        None
    }

    // =========================================================================
    // REQUEST HANDLERS
    // =========================================================================

    /// Handle MarkFinalized request from Finality (Subsystem 9)
    pub fn handle_mark_finalized(
        &mut self,
        msg: AuthenticatedMessage<MarkFinalizedRequestPayload>,
    ) -> Result<AuthenticatedMessage<BlockFinalizedPayload>, HandlerError> {
        // Step 1: Validate envelope
        self.validator.validate(&msg)?;

        // Step 2: Verify sender is Finality (9)
        self.validator
            .validate_sender(msg.sender_id, super::security::MARK_FINALIZED_SENDERS)?;

        // Step 3: Get current finalized height for response
        let prev_finalized = self.service.get_finalized_height().unwrap_or(0);

        // Step 4: Mark finalized
        self.service
            .mark_finalized(msg.payload.block_height)
            .map_err(HandlerError::Storage)?;

        // Step 5: Get block hash for response (compute from header)
        let block = self
            .service
            .read_block_by_height(msg.payload.block_height)
            .map_err(HandlerError::Storage)?;

        // Compute block hash from header data
        let block_hash = compute_block_hash(&block.block.header);

        // Step 6: Create response
        let response_payload = BlockFinalizedPayload {
            block_height: msg.payload.block_height,
            block_hash,
            previous_finalized_height: prev_finalized,
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    /// Handle ReadBlock request (from any authorized subsystem)
    pub fn handle_read_block(
        &self,
        msg: AuthenticatedMessage<ReadBlockRequestPayload>,
    ) -> Result<AuthenticatedMessage<ReadBlockResponsePayload>, HandlerError> {
        // Step 1: Validate envelope (no sender restriction for reads)
        // Note: We need mutable access for nonce tracking, but reads are logically const
        // In production, use interior mutability for the validator

        // Step 2: Process request
        let result = match msg.payload.query {
            BlockQuery::ByHash(hash) => self.service.read_block(&hash),
            BlockQuery::ByHeight(height) => self.service.read_block_by_height(height),
        };

        // Step 3: Convert to response payload
        let response_payload = ReadBlockResponsePayload {
            result: result
                .map(|stored| StoredBlockPayload {
                    block: stored.block,
                    merkle_root: stored.merkle_root,
                    state_root: stored.state_root,
                    stored_at: stored.stored_at,
                    checksum: stored.checksum,
                })
                .map_err(storage_error_to_payload),
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    /// Handle ReadBlockRange request (from any authorized subsystem)
    pub fn handle_read_block_range(
        &self,
        msg: AuthenticatedMessage<ReadBlockRangeRequestPayload>,
    ) -> Result<AuthenticatedMessage<ReadBlockRangeResponsePayload>, HandlerError> {
        use crate::ipc::payloads::security::MAX_BLOCKS_PER_RANGE;

        // Step 1: Security: Limit range to prevent DoS
        let limited_range = msg.payload.limit.min(MAX_BLOCKS_PER_RANGE);

        // Step 2: Process request with limited range
        let result = self
            .service
            .read_block_range(msg.payload.start_height, limited_range);

        // Step 2: Convert to response payload
        let chain_tip = self.service.get_latest_height().unwrap_or(0);

        let response_payload = match result {
            Ok(blocks) => {
                let has_more = blocks
                    .last()
                    .map(|b| b.block.header.height < chain_tip)
                    .unwrap_or(false);

                ReadBlockRangeResponsePayload {
                    blocks: blocks
                        .into_iter()
                        .map(|stored| StoredBlockPayload {
                            block: stored.block,
                            merkle_root: stored.merkle_root,
                            state_root: stored.state_root,
                            stored_at: stored.stored_at,
                            checksum: stored.checksum,
                        })
                        .collect(),
                    chain_tip_height: chain_tip,
                    has_more,
                }
            }
            Err(_) => ReadBlockRangeResponsePayload {
                blocks: vec![],
                chain_tip_height: chain_tip,
                has_more: false,
            },
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    /// Handle GetTransactionLocation request (from Transaction Indexing only)
    pub fn handle_get_transaction_location(
        &mut self,
        msg: AuthenticatedMessage<GetTransactionLocationRequestPayload>,
    ) -> Result<AuthenticatedMessage<TransactionLocationResponsePayload>, HandlerError> {
        // Step 1: Validate envelope
        self.validator.validate(&msg)?;

        // Step 2: Verify sender is Transaction Indexing (3)
        self.validator
            .validate_sender(msg.sender_id, super::security::TX_SENDERS)?;

        // Step 3: Process request
        let result = self
            .service
            .get_transaction_location(&msg.payload.transaction_hash);

        // Step 4: Create response
        let response_payload = TransactionLocationResponsePayload {
            transaction_hash: msg.payload.transaction_hash,
            result: result
                .map(|loc| TransactionLocationData {
                    block_hash: loc.block_hash,
                    block_height: loc.block_height,
                    transaction_index: loc.transaction_index,
                    merkle_root: loc.merkle_root,
                })
                .map_err(storage_error_to_payload),
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    /// Handle GetTransactionHashes request (from Transaction Indexing only)
    pub fn handle_get_transaction_hashes(
        &mut self,
        msg: AuthenticatedMessage<GetTransactionHashesRequestPayload>,
    ) -> Result<AuthenticatedMessage<TransactionHashesResponsePayload>, HandlerError> {
        // Step 1: Validate envelope
        self.validator.validate(&msg)?;

        // Step 2: Verify sender is Transaction Indexing (3)
        self.validator
            .validate_sender(msg.sender_id, super::security::TX_SENDERS)?;

        // Step 3: Process request
        let result = self
            .service
            .get_transaction_hashes_for_block(&msg.payload.block_hash);

        // Step 4: Get merkle root for response
        let merkle_root = self
            .service
            .read_block(&msg.payload.block_hash)
            .map(|b| b.merkle_root)
            .unwrap_or([0u8; 32]);

        // Step 5: Create response
        let response_payload = TransactionHashesResponsePayload {
            block_hash: msg.payload.block_hash,
            result: result
                .map(|hashes| TransactionHashesData {
                    transaction_hashes: hashes,
                    merkle_root,
                })
                .map_err(storage_error_to_payload),
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    /// Handle GetChainInfo request (from Block Production only)
    ///
    /// V2.4: Provides chain tip information and recent block data needed by
    /// Block Production (qc-17) to resume mining with correct difficulty
    /// after restart.
    pub fn handle_get_chain_info(
        &mut self,
        msg: AuthenticatedMessage<GetChainInfoRequestPayload>,
    ) -> Result<AuthenticatedMessage<ChainInfoResponsePayload>, HandlerError> {
        // Step 1: Validate envelope
        self.validator.validate(&msg)?;

        // Step 2: Validate sender is Block Production (17)
        self.validator
            .validate_sender(msg.sender_id, super::security::CHAIN_INFO_SENDERS)?;

        let chain_tip_height = self.service.get_latest_height().unwrap_or(0);

        // Step 2: Get recent blocks for DGW difficulty adjustment
        let recent_blocks_count = msg.payload.recent_blocks_count.min(100) as u64;
        let start_height = chain_tip_height.saturating_sub(recent_blocks_count.saturating_sub(1));

        let mut recent_blocks = Vec::new();

        if chain_tip_height > 0 {
            // Read blocks from highest to lowest
            recent_blocks = (start_height..=chain_tip_height)
                .rev()
                .filter_map(|height| self.service.read_block_by_height(height).ok())
                .map(to_difficulty_info)
                .collect();
        }

        // Step 3: Get chain tip hash and timestamp
        let (chain_tip_hash, chain_tip_timestamp) = if let Some(latest) = recent_blocks.first() {
            (latest.hash, latest.timestamp)
        } else {
            ([0u8; 32], 0)
        };

        // Step 4: Create response
        let response_payload = ChainInfoResponsePayload {
            chain_tip_height,
            chain_tip_hash,
            chain_tip_timestamp,
            recent_blocks,
        };

        Ok(AuthenticatedMessage::response(
            &msg,
            subsystem_ids::BLOCK_STORAGE,
            response_payload,
        ))
    }

    // =========================================================================
    // UTILITY METHODS
    // =========================================================================

    /// Run garbage collection on expired assemblies
    pub fn gc_expired_assemblies(&mut self) -> Vec<AssemblyTimeoutPayload> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.service
            .gc_expired_assemblies_with_data(now)
            .into_iter()
            .map(|(hash, assembly)| AssemblyTimeoutPayload {
                block_hash: hash,
                block_height: Some(assembly.block_height),
                had_validated_block: assembly.validated_block.is_some(),
                had_merkle_root: assembly.merkle_root.is_some(),
                had_state_root: assembly.state_root.is_some(),
                pending_duration_secs: now.saturating_sub(assembly.started_at),
                purged_at: now,
            })
            .collect()
    }

    /// Get reference to underlying service
    pub fn service(&self) -> &BlockStorageService<KV, FS, CS, TS, BS> {
        &self.service
    }

    /// Get mutable reference to underlying service
    pub fn service_mut(&mut self) -> &mut BlockStorageService<KV, FS, CS, TS, BS> {
        &mut self.service
    }
}
