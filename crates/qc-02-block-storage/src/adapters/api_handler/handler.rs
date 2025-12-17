//! # API Gateway Handler
//!
//! Core handler struct and methods for API Gateway integration.

use super::types::Qc02Metrics;
use crate::ports::inbound::BlockStorageApi;
use crate::adapters::security::rate_limit::{RateLimiter, RateLimitConfig, RateLimitResult};
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
    /// Rate limiter
    rate_limiter: RateLimiter,
}

impl<S: BlockStorageApi> ApiGatewayHandler<S> {
    /// Create a new API Gateway handler.
    pub fn new(service: S, disk_used_bytes: u64, disk_capacity_bytes: u64, rate_limit_config: RateLimitConfig) -> Self {
        Self {
            service,
            disk_used_bytes,
            disk_capacity_bytes,
            rate_limiter: RateLimiter::new(rate_limit_config),
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

/// API query error types.
#[derive(Debug)]
pub enum ApiQueryError {
    /// Unknown method
    UnknownMethod(String),
    /// Invalid parameters
    InvalidParams(String),
    /// Storage error
    StorageError(String),
    /// Rate limit exceeded
    RateLimitExceeded(u64),
}

impl std::fmt::Display for ApiQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownMethod(m) => write!(f, "Unknown method: {}", m),
            Self::InvalidParams(msg) => write!(f, "Invalid params: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
            Self::RateLimitExceeded(retry) => write!(f, "Rate limit exceeded. Retry after {}s", retry),
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
    // Check rate limit
    if let RateLimitResult::Limited { retry_after } = handler.rate_limiter.check() {
        return Err(ApiQueryError::RateLimitExceeded(retry_after));
    }

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
