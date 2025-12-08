//! # API Gateway Handler for Block Storage (qc-02)
//!
//! This module provides the API Gateway integration for the qc-02 Block Storage
//! subsystem, enabling the admin panel to query storage metrics and status.
//!
//! ## Supported Methods
//!
//! | Method | Description |
//! |--------|-------------|
//! | `eth_blockNumber` | Get latest block height |
//! | `eth_getBlockByNumber` | Get block by height |
//! | `eth_getBlockByHash` | Get block by hash |
//! | `debug_getBlockStorageMetrics` | Get qc-02 specific metrics |
//! | `debug_getPendingAssemblies` | Get pending assembly status |
//! | `ping` | Health check |

use crate::ports::inbound::BlockStorageApi;
use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// API Gateway handler for Block Storage subsystem.
///
/// Wraps a BlockStorageApi implementation and provides JSON-RPC compatible
/// responses for the admin panel.
pub struct ApiGatewayHandler<S: BlockStorageApi> {
    service: S,
    /// Disk usage in bytes (would be updated from filesystem adapter)
    disk_used_bytes: u64,
    /// Disk capacity in bytes
    disk_capacity_bytes: u64,
}

impl<S: BlockStorageApi> ApiGatewayHandler<S> {
    /// Create a new API Gateway handler.
    pub fn new(service: S, disk_used_bytes: u64, disk_capacity_bytes: u64) -> Self {
        Self {
            service,
            disk_used_bytes,
            disk_capacity_bytes,
        }
    }

    /// Handle `eth_blockNumber` - returns latest block height.
    pub fn handle_block_number(&self) -> serde_json::Value {
        match self.service.get_latest_height() {
            Ok(height) => serde_json::json!({
                "result": format!("0x{:x}", height)
            }),
            Err(e) => serde_json::json!({
                "error": {
                    "code": -32000,
                    "message": format!("{}", e)
                }
            }),
        }
    }

    /// Handle `eth_getBlockByNumber` - returns block by height.
    pub fn handle_get_block_by_number(&self, height: u64, full_tx: bool) -> serde_json::Value {
        match self.service.read_block_by_height(height) {
            Ok(stored) => {
                let block_hash = hex::encode(stored.block_hash());
                serde_json::json!({
                    "result": {
                        "number": format!("0x{:x}", stored.height()),
                        "hash": format!("0x{}", block_hash),
                        "parentHash": format!("0x{}", hex::encode(stored.parent_hash())),
                        "stateRoot": format!("0x{}", hex::encode(stored.state_root)),
                        "transactionsRoot": format!("0x{}", hex::encode(stored.merkle_root)),
                        "timestamp": format!("0x{:x}", stored.block.header.timestamp),
                        "transactions": if full_tx {
                            serde_json::json!(stored.block.transactions.iter()
                                .map(|tx| format!("0x{}", hex::encode(tx.tx_hash)))
                                .collect::<Vec<_>>())
                        } else {
                            serde_json::json!(stored.block.transactions.len())
                        }
                    }
                })
            }
            Err(e) => serde_json::json!({
                "error": {
                    "code": -32000,
                    "message": format!("{}", e)
                }
            }),
        }
    }

    /// Handle `eth_getBlockByHash` - returns block by hash.
    pub fn handle_get_block_by_hash(&self, hash: &Hash, full_tx: bool) -> serde_json::Value {
        match self.service.read_block(hash) {
            Ok(stored) => {
                let block_hash = hex::encode(stored.block_hash());
                serde_json::json!({
                    "result": {
                        "number": format!("0x{:x}", stored.height()),
                        "hash": format!("0x{}", block_hash),
                        "parentHash": format!("0x{}", hex::encode(stored.parent_hash())),
                        "stateRoot": format!("0x{}", hex::encode(stored.state_root)),
                        "transactionsRoot": format!("0x{}", hex::encode(stored.merkle_root)),
                        "timestamp": format!("0x{:x}", stored.block.header.timestamp),
                        "transactions": if full_tx {
                            serde_json::json!(stored.block.transactions.iter()
                                .map(|tx| format!("0x{}", hex::encode(tx.tx_hash)))
                                .collect::<Vec<_>>())
                        } else {
                            serde_json::json!(stored.block.transactions.len())
                        }
                    }
                })
            }
            Err(e) => serde_json::json!({
                "error": {
                    "code": -32000,
                    "message": format!("{}", e)
                }
            }),
        }
    }

    /// Handle `debug_getBlockStorageMetrics` - returns qc-02 specific metrics.
    pub fn handle_get_metrics(&self) -> serde_json::Value {
        let latest_height = self.service.get_latest_height().unwrap_or(0);
        let finalized_height = self.service.get_finalized_height().unwrap_or(0);
        let metadata = self.service.get_metadata().unwrap_or_default();

        let metrics = Qc02Metrics {
            latest_height,
            finalized_height,
            total_blocks: metadata.total_blocks,
            genesis_hash: metadata.genesis_hash.map(hex::encode),
            storage_version: metadata.storage_version,
            disk_used_bytes: self.disk_used_bytes,
            disk_capacity_bytes: self.disk_capacity_bytes,
            disk_usage_percent: if self.disk_capacity_bytes > 0 {
                (self.disk_used_bytes as f64 / self.disk_capacity_bytes as f64 * 100.0) as u8
            } else {
                0
            },
            // Assembly buffer metrics would be provided by the runtime
            pending_assemblies: 0,
            assembly_timeout_secs: 30,
        };

        serde_json::to_value(metrics).unwrap_or_default()
    }

    /// Handle `ping` - health check.
    pub fn handle_ping(&self) -> serde_json::Value {
        serde_json::json!({
            "result": "pong",
            "subsystem": "qc-02-block-storage",
            "latest_height": self.service.get_latest_height().unwrap_or(0),
            "finalized_height": self.service.get_finalized_height().unwrap_or(0)
        })
    }

    /// Update disk usage metrics.
    pub fn update_disk_usage(&mut self, used: u64, capacity: u64) {
        self.disk_used_bytes = used;
        self.disk_capacity_bytes = capacity;
    }

    /// Get reference to underlying service
    pub fn service(&self) -> &S {
        &self.service
    }
}

/// QC-02 specific metrics for the admin panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qc02Metrics {
    /// Latest stored block height
    pub latest_height: u64,
    /// Latest finalized block height
    pub finalized_height: u64,
    /// Total number of blocks stored
    pub total_blocks: u64,
    /// Genesis block hash (hex encoded)
    pub genesis_hash: Option<String>,
    /// Storage format version
    pub storage_version: u16,
    /// Disk space used in bytes
    pub disk_used_bytes: u64,
    /// Disk capacity in bytes
    pub disk_capacity_bytes: u64,
    /// Disk usage percentage (0-100)
    pub disk_usage_percent: u8,
    /// Number of pending block assemblies
    pub pending_assemblies: u32,
    /// Assembly timeout in seconds
    pub assembly_timeout_secs: u32,
}

/// Pending assembly info for admin panel display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPendingAssembly {
    /// Block hash (hex encoded)
    pub block_hash: String,
    /// Whether BlockValidated has been received
    pub has_validated_block: bool,
    /// Whether MerkleRootComputed has been received
    pub has_merkle_root: bool,
    /// Whether StateRootComputed has been received
    pub has_state_root: bool,
    /// When the assembly started (Unix timestamp)
    pub started_at: u64,
    /// Time elapsed in seconds
    pub elapsed_secs: u64,
}

/// API query error types.
#[derive(Debug)]
pub enum ApiQueryError {
    /// Unknown method
    UnknownMethod(String),
    /// Invalid parameters
    InvalidParams(String),
    /// Storage error
    StorageError(String),
}

impl std::fmt::Display for ApiQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMethod(m) => write!(f, "Unknown method: {}", m),
            Self::InvalidParams(msg) => write!(f, "Invalid params: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for ApiQueryError {}

/// Handle an API query from the API Gateway.
///
/// This function dispatches to the appropriate handler method based on the
/// JSON-RPC method name.
pub fn handle_api_query<S: BlockStorageApi>(
    handler: &ApiGatewayHandler<S>,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, ApiQueryError> {
    match method {
        "eth_blockNumber" => Ok(handler.handle_block_number()),

        "eth_getBlockByNumber" => {
            let height = params
                .get(0)
                .and_then(|v| v.as_str())
                .and_then(|s| {
                    if s == "latest" {
                        handler.service.get_latest_height().ok()
                    } else if s == "finalized" {
                        handler.service.get_finalized_height().ok()
                    } else if let Some(hex) = s.strip_prefix("0x") {
                        u64::from_str_radix(hex, 16).ok()
                    } else {
                        s.parse().ok()
                    }
                })
                .ok_or_else(|| ApiQueryError::InvalidParams("Invalid block number".into()))?;

            let full_tx = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

            Ok(handler.handle_get_block_by_number(height, full_tx))
        }

        "eth_getBlockByHash" => {
            let hash_str = params
                .get(0)
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiQueryError::InvalidParams("Missing block hash".into()))?;

            let hash_hex = hash_str.strip_prefix("0x").unwrap_or(hash_str);
            let hash_bytes = hex::decode(hash_hex)
                .map_err(|_| ApiQueryError::InvalidParams("Invalid hash format".into()))?;

            if hash_bytes.len() != 32 {
                return Err(ApiQueryError::InvalidParams("Hash must be 32 bytes".into()));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);

            let full_tx = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);

            Ok(handler.handle_get_block_by_hash(&hash, full_tx))
        }

        "debug_getBlockStorageMetrics" | "debug_subsystemMetrics" => {
            Ok(handler.handle_get_metrics())
        }

        "ping" => Ok(handler.handle_ping()),

        _ => Err(ApiQueryError::UnknownMethod(method.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{StorageMetadata, StoredBlock};
    use crate::domain::errors::StorageError;
    use crate::domain::value_objects::TransactionLocation;
    use shared_types::{BlockHeader, ConsensusProof, ValidatedBlock, U256};

    /// Mock service for testing
    struct MockStorageService {
        latest_height: u64,
        finalized_height: u64,
    }

    impl MockStorageService {
        fn new() -> Self {
            Self {
                latest_height: 1000,
                finalized_height: 990,
            }
        }
    }

    impl BlockStorageApi for MockStorageService {
        fn write_block(
            &mut self,
            _block: ValidatedBlock,
            _merkle_root: Hash,
            _state_root: Hash,
        ) -> Result<Hash, StorageError> {
            Ok([0; 32])
        }

        fn read_block(&self, _hash: &Hash) -> Result<StoredBlock, StorageError> {
            Err(StorageError::BlockNotFound { hash: [0; 32] })
        }

        fn read_block_by_height(&self, height: u64) -> Result<StoredBlock, StorageError> {
            if height <= self.latest_height {
                Ok(StoredBlock {
                    block: ValidatedBlock {
                        header: BlockHeader {
                            version: 1,
                            height,
                            parent_hash: [0; 32],
                            merkle_root: [0xAA; 32],
                            state_root: [0xBB; 32],
                            timestamp: 1700000000 + height,
                            proposer: [0; 32],
                            difficulty: U256::from(2).pow(U256::from(252)),
                            nonce: 0,
                        },
                        transactions: vec![],
                        consensus_proof: ConsensusProof::default(),
                    },
                    merkle_root: [0xAA; 32],
                    state_root: [0xBB; 32],
                    stored_at: 1700000000,
                    checksum: 0,
                })
            } else {
                Err(StorageError::HeightNotFound { height })
            }
        }

        fn read_block_range(
            &self,
            _start_height: u64,
            _limit: u64,
        ) -> Result<Vec<StoredBlock>, StorageError> {
            Ok(vec![])
        }

        fn mark_finalized(&mut self, _height: u64) -> Result<(), StorageError> {
            Ok(())
        }

        fn get_metadata(&self) -> Result<StorageMetadata, StorageError> {
            Ok(StorageMetadata {
                genesis_hash: Some([0; 32]),
                latest_height: self.latest_height,
                finalized_height: self.finalized_height,
                total_blocks: self.latest_height + 1,
                storage_version: 1,
            })
        }

        fn get_latest_height(&self) -> Result<u64, StorageError> {
            Ok(self.latest_height)
        }

        fn get_finalized_height(&self) -> Result<u64, StorageError> {
            Ok(self.finalized_height)
        }

        fn block_exists(&self, _hash: &Hash) -> bool {
            false
        }

        fn block_exists_at_height(&self, height: u64) -> bool {
            height <= self.latest_height
        }

        fn get_transaction_location(
            &self,
            _transaction_hash: &Hash,
        ) -> Result<TransactionLocation, StorageError> {
            Err(StorageError::TransactionNotFound { tx_hash: [0; 32] })
        }

        fn get_transaction_hashes_for_block(
            &self,
            _block_hash: &Hash,
        ) -> Result<Vec<Hash>, StorageError> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_handle_block_number() {
        let service = MockStorageService::new();
        let handler = ApiGatewayHandler::new(service, 100_000_000_000, 500_000_000_000);

        let result = handler.handle_block_number();
        assert!(result.get("result").is_some());
        assert_eq!(result["result"], "0x3e8"); // 1000 in hex
    }

    #[test]
    fn test_handle_get_metrics() {
        let service = MockStorageService::new();
        let handler = ApiGatewayHandler::new(service, 100_000_000_000, 500_000_000_000);

        let result = handler.handle_get_metrics();
        assert_eq!(result["latest_height"], 1000);
        assert_eq!(result["finalized_height"], 990);
        assert_eq!(result["disk_usage_percent"], 20);
    }

    #[test]
    fn test_handle_ping() {
        let service = MockStorageService::new();
        let handler = ApiGatewayHandler::new(service, 0, 0);

        let result = handler.handle_ping();
        assert_eq!(result["result"], "pong");
        assert_eq!(result["subsystem"], "qc-02-block-storage");
    }

    #[test]
    fn test_api_query_dispatch() {
        let service = MockStorageService::new();
        let handler = ApiGatewayHandler::new(service, 0, 0);

        // Test eth_blockNumber
        let result = handle_api_query(&handler, "eth_blockNumber", &serde_json::json!([]));
        assert!(result.is_ok());

        // Test unknown method
        let result = handle_api_query(&handler, "unknown_method", &serde_json::json!([]));
        assert!(matches!(result, Err(ApiQueryError::UnknownMethod(_))));
    }
}
