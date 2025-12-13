//! Internal response types for IPC communication per SPEC-16 Section 4.

use crate::CorrelationId;
use crate::domain::types::{Address, Bytes, Hash, SyncStatus, U256};
use serde::{Deserialize, Serialize};

/// Response envelope for all IPC messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    /// Correlation ID matching the request
    pub correlation_id: CorrelationId,
    /// Source subsystem ID that produced this response
    pub source: u8,
    /// Response payload
    pub payload: ResponsePayload,
}

/// All possible response payloads
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[allow(clippy::large_enum_variant)]
pub enum ResponsePayload {
    /// Successful response with data
    Success(SuccessData),
    /// Error response
    Error(ErrorData),
}

/// Successful response data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum SuccessData {
    // Primitive types
    Balance(U256),
    Code(Bytes),
    StorageValue(Hash),
    TransactionCount(u64),
    BlockNumber(u64),
    GasPrice(U256),
    GasEstimate(U256),
    TransactionHash(Hash),

    // Complex types
    Block(BlockData),
    Transaction(TransactionData),
    Receipt(ReceiptData),
    Logs(Vec<LogData>),
    SyncStatus(SyncStatus),

    // Pool data
    TxPoolStatus(TxPoolStatusData),
    TxPoolContent(TxPoolContentData),

    // Network data
    Peers(Vec<PeerData>),
    NodeInfo(NodeInfoData),

    // Generic
    Bool(bool),
    Null,

    /// Generic JSON value for dynamic responses
    Json(serde_json::Value),
}

/// Error response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Block data for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockData {
    pub number: u64,
    pub hash: Hash,
    pub parent_hash: Hash,
    pub nonce: u64,
    pub sha3_uncles: Hash,
    pub logs_bloom: Bytes,
    pub transactions_root: Hash,
    pub state_root: Hash,
    pub receipts_root: Hash,
    pub miner: Address,
    pub difficulty: U256,
    pub total_difficulty: U256,
    pub extra_data: Bytes,
    pub size: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<U256>,
    /// Full transactions or just hashes
    pub transactions: TransactionList,
    pub uncles: Vec<Hash>,
}

/// Transaction list - either full objects or just hashes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TransactionList {
    Hashes(Vec<Hash>),
    Full(Vec<TransactionData>),
}

/// Transaction data for responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionData {
    pub hash: Hash,
    pub nonce: u64,
    pub block_hash: Option<Hash>,
    pub block_number: Option<u64>,
    pub transaction_index: Option<u64>,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas_price: Option<U256>,
    pub gas: u64,
    pub input: Bytes,
    #[serde(rename = "type")]
    pub tx_type: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    pub v: u64,
    pub r: U256,
    pub s: U256,
}

/// Transaction receipt data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptData {
    pub transaction_hash: Hash,
    pub transaction_index: u64,
    pub block_hash: Hash,
    pub block_number: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub cumulative_gas_used: u64,
    pub effective_gas_price: U256,
    pub gas_used: u64,
    pub contract_address: Option<Address>,
    pub logs: Vec<LogData>,
    pub logs_bloom: Bytes,
    pub status: u8, // 1 = success, 0 = failure
    #[serde(rename = "type")]
    pub tx_type: u8,
}

/// Log data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogData {
    pub address: Address,
    pub topics: Vec<Hash>,
    pub data: Bytes,
    pub block_number: u64,
    pub transaction_hash: Hash,
    pub transaction_index: u64,
    pub block_hash: Hash,
    pub log_index: u64,
    pub removed: bool,
}

/// TxPool status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxPoolStatusData {
    pub pending: u64,
    pub queued: u64,
}

/// TxPool content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxPoolContentData {
    /// Pending transactions by address and nonce
    pub pending:
        std::collections::HashMap<Address, std::collections::HashMap<String, TransactionData>>,
    /// Queued transactions by address and nonce
    pub queued:
        std::collections::HashMap<Address, std::collections::HashMap<String, TransactionData>>,
}

/// Peer data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerData {
    pub id: String,
    pub name: String,
    pub enode: String,
    pub caps: Vec<String>,
    pub network: PeerNetworkData,
}

/// Peer network data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerNetworkData {
    pub local_address: String,
    pub remote_address: String,
    pub inbound: bool,
    pub trusted: bool,
    pub static_node: bool,
}

/// Node info data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeInfoData {
    pub id: String,
    pub name: String,
    pub enode: String,
    pub ip: String,
    pub ports: NodePorts,
    pub listen_addr: String,
    pub protocols: serde_json::Value,
}

/// Node ports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePorts {
    pub discovery: u16,
    pub listener: u16,
}

impl IpcResponse {
    /// Create a success response
    pub fn success(correlation_id: CorrelationId, source: u8, data: SuccessData) -> Self {
        Self {
            correlation_id,
            source,
            payload: ResponsePayload::Success(data),
        }
    }

    /// Create an error response
    pub fn error(
        correlation_id: CorrelationId,
        source: u8,
        code: i32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id,
            source,
            payload: ResponsePayload::Error(ErrorData {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// Check if response is success
    pub fn is_success(&self) -> bool {
        matches!(self.payload, ResponsePayload::Success(_))
    }

    /// Get error if present
    pub fn error_data(&self) -> Option<&ErrorData> {
        match &self.payload {
            ResponsePayload::Error(e) => Some(e),
            _ => None,
        }
    }

    /// Get success data if present
    pub fn success_data(&self) -> Option<&SuccessData> {
        match &self.payload {
            ResponsePayload::Success(d) => Some(d),
            _ => None,
        }
    }
}
