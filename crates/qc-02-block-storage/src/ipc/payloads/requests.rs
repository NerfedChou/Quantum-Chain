//! # Request Payloads
//!
//! Request payloads for IPC requests.

use shared_types::Hash;

/// Request payload types this subsystem handles
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStorageRequestPayload {
    /// Mark a block as finalized (from Finality, Subsystem 9)
    MarkFinalized(MarkFinalizedRequestPayload),
    /// Read a single block
    ReadBlock(ReadBlockRequestPayload),
    /// Read a range of blocks (for node syncing)
    ReadBlockRange(ReadBlockRangeRequestPayload),
    /// Get chain info for difficulty persistence (V2.4, from Block Production, Subsystem 17)
    GetChainInfo(GetChainInfoRequestPayload),
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

/// V2.4: Get chain info request for difficulty persistence
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetChainInfoRequestPayload {
    /// Number of recent blocks to include for DGW difficulty calculation
    pub recent_blocks_count: u32,
}

/// V2.4: Get transaction location request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionLocationRequestPayload {
    pub transaction_hash: Hash,
}

/// V2.3: Get transaction hashes request
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetTransactionHashesRequestPayload {
    pub block_hash: Hash,
}
