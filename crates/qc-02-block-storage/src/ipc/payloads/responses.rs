//! # Response Payloads
//!
//! Response and output event payloads.

use shared_types::{Hash, ValidatedBlock, U256};

/// Events emitted by Block Storage
#[derive(Debug, Clone)]
pub enum BlockStorageEventPayload {
    /// Block was successfully stored
    BlockStored(BlockStoredPayload),
    /// Block was marked as finalized
    BlockFinalized(BlockFinalizedPayload),
    /// Response to ReadBlock request
    ReadBlockResponse(Box<ReadBlockResponsePayload>),
    /// Response to ReadBlockRange request
    ReadBlockRangeResponse(ReadBlockRangeResponsePayload),
    /// Response to GetChainInfo request (V2.4)
    ChainInfoResponse(ChainInfoResponsePayload),
    /// Response to GetTransactionLocation request
    TransactionLocationResponse(TransactionLocationResponsePayload),
    /// Response to GetTransactionHashes request
    TransactionHashesResponse(TransactionHashesResponsePayload),
    /// Critical storage error
    StorageCritical(StorageCriticalPayload),
    /// Assembly timeout (V2.3 - partial block purged)
    AssemblyTimeout(AssemblyTimeoutPayload),
}

/// Block stored event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStoredPayload {
    pub block_height: u64,
    pub block_hash: Hash,
    pub merkle_root: Hash,
    pub state_root: Hash,
    pub stored_at: u64,
}

/// Block finalized event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFinalizedPayload {
    pub block_height: u64,
    pub block_hash: Hash,
    pub previous_finalized_height: u64,
}

/// Read block response
#[derive(Debug, Clone)]
pub struct ReadBlockResponsePayload {
    pub result: Result<StoredBlockPayload, StorageErrorPayload>,
}

/// Stored block data for IPC
#[derive(Debug, Clone)]
pub struct StoredBlockPayload {
    pub block: ValidatedBlock,
    pub merkle_root: Hash,
    pub state_root: Hash,
    pub stored_at: u64,
    pub checksum: u32,
}

/// Read block range response
#[derive(Debug, Clone)]
pub struct ReadBlockRangeResponsePayload {
    pub blocks: Vec<StoredBlockPayload>,
    pub chain_tip_height: u64,
    pub has_more: bool,
}

/// V2.4: Chain info response for difficulty persistence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainInfoResponsePayload {
    pub chain_tip_height: u64,
    pub chain_tip_hash: Hash,
    pub chain_tip_timestamp: u64,
    pub recent_blocks: Vec<BlockDifficultyInfo>,
}

/// V2.4: Minimal block info for difficulty adjustment
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDifficultyInfo {
    pub height: u64,
    pub timestamp: u64,
    pub difficulty: U256,
    pub hash: Hash,
}

/// V2.3: Transaction location response
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocationResponsePayload {
    pub transaction_hash: Hash,
    pub result: Result<TransactionLocationData, StorageErrorPayload>,
}

/// V2.3: Transaction location data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionLocationData {
    pub block_hash: Hash,
    pub block_height: u64,
    pub transaction_index: usize,
    pub merkle_root: Hash,
}

/// V2.3: Transaction hashes response
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesResponsePayload {
    pub block_hash: Hash,
    pub result: Result<TransactionHashesData, StorageErrorPayload>,
}

/// V2.3: Transaction hashes data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionHashesData {
    pub transaction_hashes: Vec<Hash>,
    pub merkle_root: Hash,
}

/// Storage error payload for IPC
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageErrorPayload {
    pub error_type: StorageErrorType,
    pub message: String,
    pub block_hash: Option<Hash>,
    pub block_height: Option<u64>,
}

/// Storage error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageErrorType {
    BlockNotFound,
    HeightNotFound,
    DataCorruption,
    DatabaseError,
    TransactionNotFound,
    DiskFull,
    UnauthorizedSender,
}

/// Critical storage error
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCriticalPayload {
    pub error_type: CriticalErrorType,
    pub message: String,
    pub affected_block: Option<Hash>,
    pub affected_height: Option<u64>,
    pub timestamp: u64,
    pub requires_manual_intervention: bool,
}

/// Critical error types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriticalErrorType {
    DiskFull,
    DataCorruption,
    DatabaseFailure,
    IOFailure,
}

/// Assembly timeout event (V2.3)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssemblyTimeoutPayload {
    pub block_hash: Hash,
    pub block_height: Option<u64>,
    pub had_validated_block: bool,
    pub had_merkle_root: bool,
    pub had_state_root: bool,
    pub pending_duration_secs: u64,
    pub purged_at: u64,
}
