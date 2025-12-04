//! Internal request types for IPC communication per SPEC-16 Section 4.
//!
//! CRITICAL: Read-only requests have NO signatures (internal trusted channels).
//! Only SubmitTransaction includes user's transaction signature.

use crate::domain::correlation::CorrelationId;
use crate::domain::types::{Address, BlockId, Bytes, CallRequest, Filter, Hash, U256};
use serde::{Deserialize, Serialize};

/// Request envelope for all IPC messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    /// Correlation ID for response matching
    pub correlation_id: CorrelationId,
    /// Target subsystem
    pub target: String,
    /// Request payload
    pub payload: RequestPayload,
}

/// All possible request payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum RequestPayload {
    // ═══════════════════════════════════════════════════════════════════════
    // STATE QUERIES → qc-04-state-management
    // ═══════════════════════════════════════════════════════════════════════
    GetBalance(GetBalanceRequest),
    GetCode(GetCodeRequest),
    GetStorageAt(GetStorageAtRequest),
    GetTransactionCount(GetTransactionCountRequest),

    // ═══════════════════════════════════════════════════════════════════════
    // BLOCK QUERIES → qc-02-block-storage
    // ═══════════════════════════════════════════════════════════════════════
    GetBlockByHash(GetBlockByHashRequest),
    GetBlockByNumber(GetBlockByNumberRequest),
    GetBlockNumber(GetBlockNumberRequest),
    GetFeeHistory(GetFeeHistoryRequest),

    // ═══════════════════════════════════════════════════════════════════════
    // TRANSACTION QUERIES → qc-03-transaction-indexing
    // ═══════════════════════════════════════════════════════════════════════
    GetTransactionByHash(GetTransactionByHashRequest),
    GetTransactionReceipt(GetTransactionReceiptRequest),
    GetLogs(GetLogsRequest),
    GetBlockReceipts(GetBlockReceiptsRequest),

    // ═══════════════════════════════════════════════════════════════════════
    // EXECUTION → qc-11-smart-contracts
    // ═══════════════════════════════════════════════════════════════════════
    Call(CallRequestPayload),
    EstimateGas(EstimateGasRequest),

    // ═══════════════════════════════════════════════════════════════════════
    // MEMPOOL → qc-06-mempool
    // ═══════════════════════════════════════════════════════════════════════
    SubmitTransaction(SubmitTransactionRequest),
    GetGasPrice(GetGasPriceRequest),
    GetMaxPriorityFeePerGas(GetMaxPriorityFeePerGasRequest),
    GetTxPoolStatus(GetTxPoolStatusRequest),
    GetTxPoolContent(GetTxPoolContentRequest),

    // ═══════════════════════════════════════════════════════════════════════
    // NETWORK → qc-07-network
    // ═══════════════════════════════════════════════════════════════════════
    GetPeers(GetPeersRequest),
    GetNodeInfo(GetNodeInfoRequest),
    GetSyncStatus(GetSyncStatusRequest),
    AddPeer(AddPeerRequest),
    RemovePeer(RemovePeerRequest),
}

// ═══════════════════════════════════════════════════════════════════════════
// STATE QUERY REQUESTS (NO SIGNATURES - trusted internal channel)
// ═══════════════════════════════════════════════════════════════════════════

/// Get account balance request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBalanceRequest {
    pub address: Address,
    pub block_id: BlockId,
}

/// Get contract code request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCodeRequest {
    pub address: Address,
    pub block_id: BlockId,
}

/// Get storage at position request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetStorageAtRequest {
    pub address: Address,
    pub position: U256,
    pub block_id: BlockId,
}

/// Get account nonce request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionCountRequest {
    pub address: Address,
    pub block_id: BlockId,
}

// ═══════════════════════════════════════════════════════════════════════════
// BLOCK QUERY REQUESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Get block by hash request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockByHashRequest {
    pub hash: Hash,
    pub include_transactions: bool,
}

/// Get block by number request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockByNumberRequest {
    pub block_id: BlockId,
    pub include_transactions: bool,
}

/// Get current block number request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockNumberRequest;

/// Get fee history request (EIP-1559)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFeeHistoryRequest {
    /// Number of blocks to return (max 1024)
    pub block_count: u64,
    /// Newest block in the range
    pub newest_block: BlockId,
    /// Percentiles for priority fee calculation (0-100)
    pub reward_percentiles: Option<Vec<f64>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// TRANSACTION QUERY REQUESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Get transaction by hash request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionByHashRequest {
    pub hash: Hash,
}

/// Get transaction receipt request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTransactionReceiptRequest {
    pub hash: Hash,
}

/// Get logs request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetLogsRequest {
    pub filter: Filter,
}

/// Get all receipts for a block
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetBlockReceiptsRequest {
    pub block_id: BlockId,
}

// ═══════════════════════════════════════════════════════════════════════════
// EXECUTION REQUESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Call request (eth_call)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRequestPayload {
    pub call: CallRequest,
    pub block_id: BlockId,
}

/// Estimate gas request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateGasRequest {
    pub call: CallRequest,
    pub block_id: Option<BlockId>,
}

// ═══════════════════════════════════════════════════════════════════════════
// MEMPOOL REQUESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Submit transaction request
///
/// CRITICAL: This is the ONLY request that includes signature data.
/// The signature is the USER's signature on the transaction, NOT an internal auth signature.
///
/// The raw_transaction has been pre-validated for RLP structure before reaching here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitTransactionRequest {
    /// Pre-validated RLP-encoded transaction bytes
    pub raw_transaction: Bytes,
    /// Pre-computed transaction hash
    pub tx_hash: Hash,
    /// Sender address recovered from signature
    pub sender: Address,
    /// Nonce from transaction
    pub nonce: u64,
    /// Gas price from transaction (for prioritization)
    pub gas_price: U256,
    /// Gas limit from transaction
    pub gas_limit: u64,
}

/// Get gas price request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetGasPriceRequest;

/// Get max priority fee per gas request (EIP-1559)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetMaxPriorityFeePerGasRequest;

/// Get txpool status request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTxPoolStatusRequest;

/// Get txpool content request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTxPoolContentRequest {
    /// Optional address filter
    pub address: Option<Address>,
}

// ═══════════════════════════════════════════════════════════════════════════
// NETWORK REQUESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Get peers request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPeersRequest;

/// Get node info request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetNodeInfoRequest;

/// Get sync status request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSyncStatusRequest;

/// Add peer request (admin only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPeerRequest {
    pub enode_url: String,
}

/// Remove peer request (admin only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovePeerRequest {
    pub enode_url: String,
}

impl IpcRequest {
    /// Create a new IPC request
    pub fn new(target: impl Into<String>, payload: RequestPayload) -> Self {
        Self {
            correlation_id: CorrelationId::new(),
            target: target.into(),
            payload,
        }
    }

    /// Create with specific correlation ID
    pub fn with_correlation_id(
        correlation_id: CorrelationId,
        target: impl Into<String>,
        payload: RequestPayload,
    ) -> Self {
        Self {
            correlation_id,
            target: target.into(),
            payload,
        }
    }
}
