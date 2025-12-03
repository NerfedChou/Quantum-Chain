//! IPC Payload definitions per SPEC-02 Section 4
//!
//! All payloads follow Envelope-Only Identity (v2.2):
//! - NO `sender_id` or `requester_id` fields in payloads
//! - Identity derived ONLY from AuthenticatedMessage envelope

use shared_types::{Hash, ValidatedBlock};

// ============================================================
// INCOMING EVENT PAYLOADS (V2.3 Choreography)
// ============================================================

/// BlockValidated event from Consensus (Subsystem 8)
///
/// Block Storage buffers this until MerkleRootComputed and StateRootComputed arrive.
#[derive(Debug, Clone)]
pub struct BlockValidatedPayload {
    /// The consensus-validated block
    pub block: ValidatedBlock,
    /// Block hash for correlation with other events
    pub block_hash: Hash,
    /// Block height for ordering
    pub block_height: u64,
}

/// MerkleRootComputed event from Transaction Indexing (Subsystem 3)
///
/// Block Storage buffers this until BlockValidated and StateRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The computed Merkle root of transactions
    pub merkle_root: Hash,
}

/// StateRootComputed event from State Management (Subsystem 4)
///
/// Block Storage buffers this until BlockValidated and MerkleRootComputed arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateRootComputedPayload {
    /// Block hash to correlate with other components
    pub block_hash: Hash,
    /// The state root after executing this block
    pub state_root: Hash,
}

// ============================================================
// REQUEST PAYLOADS
// ============================================================

/// Request payload types this subsystem handles
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStorageRequestPayload {
    /// Mark a block as finalized (from Finality, Subsystem 9)
    MarkFinalized(MarkFinalizedRequestPayload),
    /// Read a single block
    ReadBlock(ReadBlockRequestPayload),
    /// Read a range of blocks (for node syncing)
    ReadBlockRange(ReadBlockRangeRequestPayload),
    /// Get transaction location (V2.3, from Tx Indexing, Subsystem 3)
    GetTransactionLocation(GetTransactionLocationRequestPayload),
    /// Get transaction hashes for a block (V2.3, from Tx Indexing, Subsystem 3)
    GetTransactionHashes(GetTransactionHashesRequestPayload),
}

/// Mark finalized request from Finality subsystem
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkFinalizedRequestPayload {
    pub block_height: u64,
    pub finality_proof: FinalityProof,
}

/// Finality proof structure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalityProof {
    pub block_hash: Hash,
    pub signatures: Vec<ValidatorSignature>,
    pub timestamp: u64,
}

/// Validator signature
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorSignature {
    pub validator_id: [u8; 32],
    pub signature: [u8; 64],
}

/// Read block request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockRequestPayload {
    pub query: BlockQuery,
}

/// Block query type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockQuery {
    ByHash(Hash),
    ByHeight(u64),
}

/// Read block range request for efficient batch reads
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlockRangeRequestPayload {
    /// First block height to read (inclusive)
    pub start_height: u64,
    /// Maximum number of blocks to return (capped at 100)
    pub limit: u64,
}

/// V2.3: Get transaction location request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionLocationRequestPayload {
    pub transaction_hash: Hash,
}

/// V2.3: Get transaction hashes request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionHashesRequestPayload {
    pub block_hash: Hash,
}

// ============================================================
// RESPONSE/EVENT PAYLOADS
// ============================================================

/// Events emitted by Block Storage
#[derive(Debug, Clone)]
pub enum BlockStorageEventPayload {
    /// Block was successfully stored
    BlockStored(BlockStoredPayload),
    /// Block was marked as finalized
    BlockFinalized(BlockFinalizedPayload),
    /// Response to ReadBlock request
    ReadBlockResponse(ReadBlockResponsePayload),
    /// Response to ReadBlockRange request
    ReadBlockRangeResponse(ReadBlockRangeResponsePayload),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_query_variants() {
        let by_hash = BlockQuery::ByHash([0xAB; 32]);
        let by_height = BlockQuery::ByHeight(100);

        match by_hash {
            BlockQuery::ByHash(h) => assert_eq!(h, [0xAB; 32]),
            _ => panic!("Expected ByHash"),
        }

        match by_height {
            BlockQuery::ByHeight(h) => assert_eq!(h, 100),
            _ => panic!("Expected ByHeight"),
        }
    }

    #[test]
    fn test_storage_error_payload() {
        let error = StorageErrorPayload {
            error_type: StorageErrorType::BlockNotFound,
            message: "Block not found".into(),
            block_hash: Some([0xAB; 32]),
            block_height: None,
        };

        assert_eq!(error.error_type, StorageErrorType::BlockNotFound);
        assert!(error.block_hash.is_some());
    }
}
