//! # API Gateway Handler
//!
//! Adapter for handling API queries from qc-16 (API Gateway).
//! This provides metrics and stats for the admin panel via the gateway.
//!
//! ## Architecture
//!
//! ```text
//! Admin Panel → qc-16 (API Gateway) → qc-03 ApiGatewayHandler → TransactionIndexingApi
//! ```
//!
//! All admin panel requests flow through qc-16, never directly to subsystems.

use crate::ports::TransactionIndexingApi;
use serde::{Deserialize, Serialize};
use shared_types::Hash;

/// Error from API query handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiQueryError {
    pub code: i32,
    pub message: String,
}

impl ApiQueryError {
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {}", method),
        }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self {
            code: -32602,
            message: msg.to_string(),
        }
    }
}

impl std::fmt::Display for ApiQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ApiQueryError {}

/// Metrics structure for qc-03 admin panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qc03Metrics {
    /// Total transactions indexed
    pub total_indexed: u64,
    /// Number of Merkle trees currently cached
    pub cached_trees: usize,
    /// Maximum trees allowed in cache (INVARIANT-5)
    pub max_cached_trees: usize,
    /// Cache utilization percentage
    pub cache_utilization_percent: f64,
    /// Number of proofs generated
    pub proofs_generated: u64,
    /// Number of proofs verified
    pub proofs_verified: u64,
    /// Last merkle root computed (hex string)
    pub last_merkle_root: Option<String>,
    /// Last block height indexed
    pub last_block_height: Option<u64>,
    /// Average tree depth (log2 of typical transaction count)
    pub avg_tree_depth: Option<u8>,
    /// Proof generation success rate (if available)
    pub proof_success_rate: Option<f64>,
    /// Head lag (blocks behind chain tip)
    pub head_lag: u64,
    /// Sync speed (blocks per second)
    pub sync_speed: f64,
    /// End-to-end latency in milliseconds
    pub e2e_latency_ms: u64,
    /// Traffic pattern: tx count per block (last N blocks)
    pub block_tx_counts: Vec<BlockTxCount>,
}

/// Transaction count for a specific block (traffic pattern data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTxCount {
    pub block: u64,
    pub tx_count: u64,
}

/// Response for transaction location lookup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionLocationResponse {
    pub found: bool,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
    pub tx_index: Option<usize>,
    pub merkle_root: Option<String>,
}

/// API Gateway handler for qc-03.
///
/// This adapter handles requests from qc-16 (API Gateway) for:
/// - Subsystem metrics (admin panel)
/// - Health checks
/// - Transaction location queries
pub struct ApiGatewayHandler<S> {
    service: S,
    /// Last merkle root computed (tracked for metrics)
    last_merkle_root: Option<Hash>,
    /// Last block height indexed
    last_block_height: Option<u64>,
}

impl<S: TransactionIndexingApi> ApiGatewayHandler<S> {
    /// Create a new API handler.
    pub fn new(service: S) -> Self {
        Self {
            service,
            last_merkle_root: None,
            last_block_height: None,
        }
    }

    /// Get mutable access to the service.
    pub fn service_mut(&mut self) -> &mut S {
        &mut self.service
    }

    /// Update last merkle root (called after indexing a block)
    pub fn set_last_merkle_root(&mut self, root: Hash, block_height: u64) {
        self.last_merkle_root = Some(root);
        self.last_block_height = Some(block_height);
    }

    /// Handle get_metrics request (debug panel).
    ///
    /// Returns comprehensive metrics for the admin panel.
    pub fn handle_get_metrics(&self) -> serde_json::Value {
        let stats = self.service.get_stats();

        let cache_utilization = if stats.max_cached_trees > 0 {
            (stats.cached_trees as f64 / stats.max_cached_trees as f64) * 100.0
        } else {
            0.0
        };

        // Calculate proof success rate
        let total_proof_attempts = stats.proofs_generated + stats.proofs_verified;
        let proof_success_rate = if total_proof_attempts > 0 {
            Some((stats.proofs_verified as f64 / total_proof_attempts as f64) * 100.0)
        } else {
            None
        };

        // Use avg_tree_depth from stats, or estimate if not available
        let avg_tree_depth = if stats.avg_tree_depth > 0 {
            Some(stats.avg_tree_depth)
        } else if stats.total_indexed_txs > 0 && stats.cached_trees > 0 {
            let avg_txs = stats.total_indexed_txs as f64 / stats.cached_trees as f64;
            Some((avg_txs.max(1.0).log2().ceil() as u8).max(1))
        } else {
            None
        };

        let metrics = Qc03Metrics {
            total_indexed: stats.total_indexed_txs,
            cached_trees: stats.cached_trees,
            max_cached_trees: stats.max_cached_trees,
            cache_utilization_percent: cache_utilization,
            proofs_generated: stats.proofs_generated,
            proofs_verified: stats.proofs_verified,
            last_merkle_root: stats.last_merkle_root.map(|h| hex::encode(&h[..8])),
            last_block_height: Some(stats.last_indexed_height),
            avg_tree_depth,
            proof_success_rate,
            head_lag: 0, // Will be calculated by caller with chain tip info
            sync_speed: stats.blocks_per_second,
            e2e_latency_ms: stats.e2e_latency_ms,
            block_tx_counts: Vec::new(), // Will be populated by caller
        };

        serde_json::to_value(metrics).unwrap_or_default()
    }

    /// Handle ping request (health check).
    pub fn handle_ping(&self) -> serde_json::Value {
        serde_json::json!({
            "status": "ok",
            "subsystem": "qc-03-transaction-indexing"
        })
    }

    /// Handle get_transaction_location request.
    pub fn handle_get_location(&self, tx_hash: Hash) -> serde_json::Value {
        match self.service.get_transaction_location(tx_hash) {
            Ok(location) => {
                let response = TransactionLocationResponse {
                    found: true,
                    block_height: Some(location.block_height),
                    block_hash: Some(hex::encode(&location.block_hash[..8])),
                    tx_index: Some(location.tx_index),
                    merkle_root: Some(hex::encode(&location.merkle_root[..8])),
                };
                serde_json::to_value(response).unwrap_or_default()
            }
            Err(_) => {
                let response = TransactionLocationResponse {
                    found: false,
                    block_height: None,
                    block_hash: None,
                    tx_index: None,
                    merkle_root: None,
                };
                serde_json::to_value(response).unwrap_or_default()
            }
        }
    }

    /// Handle is_indexed check.
    pub fn handle_is_indexed(&self, tx_hash: Hash) -> serde_json::Value {
        serde_json::json!({
            "indexed": self.service.is_indexed(tx_hash)
        })
    }
}

/// Handle an API query from qc-16.
///
/// ## Supported Methods
///
/// - `ping`: Health check
/// - `get_metrics`: Get subsystem metrics for admin panel
/// - `get_location`: Get transaction location by hash
/// - `is_indexed`: Check if transaction is indexed
pub fn handle_api_query<S: TransactionIndexingApi>(
    handler: &ApiGatewayHandler<S>,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, ApiQueryError> {
    match method {
        "ping" => Ok(handler.handle_ping()),
        "get_metrics" => Ok(handler.handle_get_metrics()),
        "get_location" => {
            let tx_hash = parse_hash_param(params, "tx_hash")?;
            Ok(handler.handle_get_location(tx_hash))
        }
        "is_indexed" => {
            let tx_hash = parse_hash_param(params, "tx_hash")?;
            Ok(handler.handle_is_indexed(tx_hash))
        }
        _ => Err(ApiQueryError::method_not_found(method)),
    }
}

/// Parse a hash parameter from JSON params.
fn parse_hash_param(params: &serde_json::Value, name: &str) -> Result<Hash, ApiQueryError> {
    let hex_str = params
        .get(name)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiQueryError::invalid_params(&format!("Missing {} parameter", name)))?;

    let bytes = hex::decode(hex_str.trim_start_matches("0x"))
        .map_err(|_| ApiQueryError::invalid_params("Invalid hex format"))?;

    if bytes.len() != 32 {
        return Err(ApiQueryError::invalid_params("Hash must be 32 bytes"));
    }

    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IndexingError, IndexingStats, MerkleProof, TransactionLocation};

    struct MockIndexingService {
        stats: IndexingStats,
    }

    impl MockIndexingService {
        fn new() -> Self {
            Self {
                stats: IndexingStats {
                    total_indexed_txs: 1000,
                    cached_trees: 50,
                    max_cached_trees: 100,
                    proofs_generated: 200,
                    proofs_verified: 180,
                    last_indexed_height: 500,
                    avg_tree_depth: 7,
                    blocks_per_second: 12.5,
                    e2e_latency_ms: 250,
                    last_merkle_root: None,
                },
            }
        }
    }

    impl TransactionIndexingApi for MockIndexingService {
        fn generate_proof(&mut self, _tx_hash: Hash) -> Result<MerkleProof, IndexingError> {
            Err(IndexingError::TransactionNotFound { tx_hash: [0; 32] })
        }

        fn verify_proof(&self, _proof: &MerkleProof) -> bool {
            true
        }

        fn get_transaction_location(&self, _tx_hash: Hash) -> Result<TransactionLocation, IndexingError> {
            Err(IndexingError::TransactionNotFound { tx_hash: [0; 32] })
        }

        fn is_indexed(&self, _tx_hash: Hash) -> bool {
            false
        }

        fn get_stats(&self) -> IndexingStats {
            self.stats.clone()
        }
    }

    #[test]
    fn test_handle_ping() {
        let service = MockIndexingService::new();
        let handler = ApiGatewayHandler::new(service);

        let result = handler.handle_ping();
        assert_eq!(result["status"], "ok");
        assert_eq!(result["subsystem"], "qc-03-transaction-indexing");
    }

    #[test]
    fn test_handle_get_metrics() {
        let service = MockIndexingService::new();
        let handler = ApiGatewayHandler::new(service);

        let result = handler.handle_get_metrics();
        assert_eq!(result["total_indexed"], 1000);
        assert_eq!(result["cached_trees"], 50);
        assert_eq!(result["max_cached_trees"], 100);
        assert_eq!(result["proofs_generated"], 200);
        assert_eq!(result["proofs_verified"], 180);
        assert_eq!(result["last_block_height"], 500);
        assert_eq!(result["sync_speed"], 12.5);
        assert_eq!(result["e2e_latency_ms"], 250);
    }

    #[test]
    fn test_handle_api_query_ping() {
        let service = MockIndexingService::new();
        let handler = ApiGatewayHandler::new(service);

        let result = handle_api_query(&handler, "ping", &serde_json::Value::Null);
        assert!(result.is_ok());
    }

    #[test]
    fn test_handle_api_query_unknown() {
        let service = MockIndexingService::new();
        let handler = ApiGatewayHandler::new(service);

        let result = handle_api_query(&handler, "unknown_method", &serde_json::Value::Null);
        assert!(result.is_err());
    }
}
