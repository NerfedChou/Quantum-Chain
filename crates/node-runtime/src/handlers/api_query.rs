//! # API Query Handler
//!
//! Listens for `ApiQuery` events from the Event Bus and routes them to
//! the appropriate subsystem, then publishes `ApiQueryResponse` events.
//!
//! ## Architecture (Hexagonal + EDA)
//!
//! This handler is the **orchestrator** that connects qc-16 (API Gateway)
//! to internal subsystems via the Event Bus. It follows the choreography
//! pattern - it doesn't call subsystems directly, but publishes/subscribes
//! to events.
//!
//! ## Query Flow
//!
//! ```text
//! qc-16 API Gateway
//!       │
//!       │ publishes ApiQuery
//!       ▼
//! ┌─────────────────┐
//! │  Event Bus      │
//! └─────────────────┘
//!       │
//!       │ ApiQueryHandler subscribes
//!       ▼
//! ┌─────────────────────────────────────┐
//! │  ApiQueryHandler                    │
//! │  - Routes by target subsystem       │
//! │  - Calls subsystem API              │
//! │  - Publishes ApiQueryResponse       │
//! └─────────────────────────────────────┘
//!       │
//!       │ publishes ApiQueryResponse
//!       ▼
//! ┌─────────────────┐
//! │  Event Bus      │
//! └─────────────────┘
//!       │
//!       │ qc-16 receives response
//!       ▼
//! qc-16 API Gateway
//! ```

use crate::container::SubsystemContainer;
use shared_bus::{
    ApiQueryError, BlockchainEvent, EventFilter, EventPublisher, EventTopic, Subscription,
};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Handler that processes API queries from the API Gateway.
///
/// Subscribes to `ApiQuery` events and routes them to the appropriate
/// subsystem, then publishes `ApiQueryResponse` events.
pub struct ApiQueryHandler {
    /// Reference to the subsystem container
    container: Arc<SubsystemContainer>,
    /// Event bus subscription for receiving queries
    subscription: Subscription,
}

impl ApiQueryHandler {
    /// Create a new API query handler.
    ///
    /// Subscribes to the ApiGateway topic on the event bus.
    pub fn new(container: Arc<SubsystemContainer>) -> Self {
        // Subscribe to ApiGateway topic to receive ApiQuery events
        let filter = EventFilter::topics(vec![EventTopic::ApiGateway]);
        let subscription = container.event_bus.subscribe(filter);

        Self {
            container,
            subscription,
        }
    }

    /// Start processing queries.
    ///
    /// This runs in a loop, receiving queries and dispatching responses.
    /// Should be spawned as a background task.
    #[instrument(skip(self), name = "api_query_handler")]
    pub async fn run(mut self) {
        info!("[ApiQueryHandler] Started listening for API queries");

        loop {
            match self.subscription.recv().await {
                Some(BlockchainEvent::ApiQuery {
                    correlation_id,
                    target,
                    method,
                    params,
                }) => {
                    debug!(
                        correlation_id = %correlation_id,
                        target = %target,
                        method = %method,
                        "Received API query"
                    );

                    // Process the query and get result
                    let result = self.process_query(&target, &method, &params).await;

                    // Determine source subsystem ID from target
                    let source = Self::target_to_subsystem_id(&target);

                    // Publish response
                    let response = BlockchainEvent::ApiQueryResponse {
                        correlation_id: correlation_id.clone(),
                        source,
                        result,
                    };

                    let receivers = self.container.event_bus.publish(response).await;
                    debug!(
                        correlation_id = %correlation_id,
                        receivers = receivers,
                        "Published API query response"
                    );
                }
                Some(other) => {
                    // Received a non-query event (shouldn't happen with proper filtering)
                    warn!("Received unexpected event type: {:?}", other);
                }
                None => {
                    // Event bus closed
                    error!("[ApiQueryHandler] Event bus closed, shutting down");
                    break;
                }
            }
        }
    }

    /// Process a query and return the result.
    async fn process_query(
        &self,
        target: &str,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        match target {
            "qc-02-block-storage" => self.handle_block_storage_query(method, params).await,
            "qc-06-mempool" => self.handle_mempool_query(method, params).await,
            "qc-01-peer-discovery" => self.handle_peer_discovery_query(method, params).await,
            "qc-04-state-management" => self.handle_state_management_query(method, params).await,
            "node-runtime" => self.handle_node_runtime_query(method, params).await,
            "admin" => self.handle_admin_query(method, params).await,
            _ => {
                warn!(target = %target, "Unknown query target");
                Err(ApiQueryError {
                    code: -32601,
                    message: format!("Unknown target subsystem: {}", target),
                })
            }
        }
    }

    /// Handle queries for qc-02 Block Storage.
    async fn handle_block_storage_query(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        use qc_02_block_storage::BlockStorageApi;

        match method {
            "get_block_number" => {
                let storage = self.container.block_storage.read();
                match storage.get_latest_height() {
                    Ok(height) => Ok(serde_json::json!(format!("0x{:x}", height))),
                    Err(e) => Err(ApiQueryError {
                        code: -32000,
                        message: format!("Failed to get block height: {}", e),
                    }),
                }
            }
            "get_block_by_number" => {
                // Parse block_id from params
                let block_id = params
                    .get("GetBlockByNumber")
                    .and_then(|v| v.get("block_id"));
                
                let storage = self.container.block_storage.read();
                
                // Get the height to query
                let height = if let Some(id) = block_id {
                    // Parse block ID (latest, pending, or number)
                    if let Some(tag) = id.as_str() {
                        match tag {
                            "latest" | "pending" => storage.get_latest_height().unwrap_or(0),
                            "earliest" => 0,
                            hex if hex.starts_with("0x") => {
                                u64::from_str_radix(&hex[2..], 16).unwrap_or(0)
                            }
                            _ => 0,
                        }
                    } else if let Some(num) = id.as_u64() {
                        num
                    } else {
                        storage.get_latest_height().unwrap_or(0)
                    }
                } else {
                    storage.get_latest_height().unwrap_or(0)
                };

                match storage.read_block_by_height(height) {
                    Ok(stored) => {
                        // Convert to Ethereum-compatible block format
                        let block = &stored.block;
                        Ok(serde_json::json!({
                            "number": format!("0x{:x}", block.header.height),
                            "hash": format!("0x{}", hex::encode(stored.block.header.parent_hash)),
                            "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
                            "timestamp": format!("0x{:x}", block.header.timestamp),
                            "stateRoot": format!("0x{}", hex::encode(stored.state_root)),
                            "transactionsRoot": format!("0x{}", hex::encode(stored.merkle_root)),
                            "receiptsRoot": format!("0x{}", hex::encode([0u8; 32])),
                            "miner": format!("0x{}", hex::encode(&block.header.proposer[..20])),
                            "gasLimit": "0x1c9c380",
                            "gasUsed": "0x0",
                            "transactions": block.transactions.iter().map(|tx| {
                                format!("0x{}", hex::encode(&tx.tx_hash))
                            }).collect::<Vec<_>>(),
                            "size": "0x0"
                        }))
                    }
                    Err(_) => Ok(serde_json::Value::Null),
                }
            }
            "get_block_by_hash" => {
                // Parse hash from params
                let hash_hex = params
                    .get("GetBlockByHash")
                    .and_then(|v| v.get("hash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                // Parse hex string to [u8; 32]
                let hash_bytes = if hash_hex.starts_with("0x") {
                    hex::decode(&hash_hex[2..]).unwrap_or_default()
                } else {
                    hex::decode(hash_hex).unwrap_or_default()
                };
                
                if hash_bytes.len() != 32 {
                    return Ok(serde_json::Value::Null);
                }
                
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&hash_bytes);
                
                let storage = self.container.block_storage.read();
                match storage.read_block(&hash) {
                    Ok(stored) => {
                        let block = &stored.block;
                        Ok(serde_json::json!({
                            "number": format!("0x{:x}", block.header.height),
                            "hash": format!("0x{}", hex::encode(hash)),
                            "parentHash": format!("0x{}", hex::encode(block.header.parent_hash)),
                            "timestamp": format!("0x{:x}", block.header.timestamp),
                            "stateRoot": format!("0x{}", hex::encode(stored.state_root)),
                            "transactionsRoot": format!("0x{}", hex::encode(stored.merkle_root)),
                            "receiptsRoot": format!("0x{}", hex::encode([0u8; 32])),
                            "miner": format!("0x{}", hex::encode(&block.header.proposer[..20])),
                            "gasLimit": "0x1c9c380",
                            "gasUsed": "0x0",
                            "transactions": block.transactions.iter().map(|tx| {
                                format!("0x{}", hex::encode(&tx.tx_hash))
                            }).collect::<Vec<_>>(),
                            "size": "0x0"
                        }))
                    }
                    Err(_) => Ok(serde_json::Value::Null),
                }
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown block storage method: {}", method),
            }),
        }
    }

    /// Handle queries for qc-06 Mempool.
    async fn handle_mempool_query(
        &self,
        method: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        match method {
            "get_gas_price" => {
                // Return the configured minimum gas price from mempool
                // For now, return a default value (1 gwei = 10^9 wei)
                let gas_price = 1_000_000_000u64;
                Ok(serde_json::json!(format!("0x{:x}", gas_price)))
            }
            "get_txpool_status" => {
                let pool = self.container.mempool.read();
                let pending = pool.pending_count();
                Ok(serde_json::json!({
                    "pending": format!("0x{:x}", pending),
                    "queued": "0x0"
                }))
            }
            "get_txpool_content" => {
                let pool = self.container.mempool.read();
                // Get all pending transactions (use large max to get all)
                let pending_txs = pool.get_for_block(10000, u64::MAX);
                
                // Group transactions by sender address
                let mut pending_by_sender: std::collections::HashMap<String, std::collections::HashMap<String, serde_json::Value>> = 
                    std::collections::HashMap::new();
                
                for tx in pending_txs {
                    let sender = format!("0x{}", hex::encode(&tx.sender));
                    let nonce = format!("0x{:x}", tx.nonce);
                    let tx_data = serde_json::json!({
                        "hash": format!("0x{}", hex::encode(&tx.hash)),
                        "nonce": nonce.clone(),
                        "gasPrice": format!("0x{}", tx.gas_price.to_string()),
                        "gas": format!("0x{:x}", tx.gas_limit),
                        "to": tx.transaction.to.map(|addr| format!("0x{}", hex::encode(addr))),
                        "value": format!("0x{}", tx.transaction.value.to_string()),
                        "input": format!("0x{}", hex::encode(&tx.transaction.data))
                    });
                    
                    pending_by_sender
                        .entry(sender)
                        .or_default()
                        .insert(nonce, tx_data);
                }
                
                Ok(serde_json::json!({
                    "pending": pending_by_sender,
                    "queued": {}
                }))
            }
            "get_max_priority_fee_per_gas" => {
                // Return suggested priority fee (0.1 gwei)
                let priority_fee = 100_000_000u64;
                Ok(serde_json::json!(format!("0x{:x}", priority_fee)))
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown mempool method: {}", method),
            }),
        }
    }

    /// Handle queries for qc-01 Peer Discovery.
    async fn handle_peer_discovery_query(
        &self,
        method: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        use qc_01_peer_discovery::PeerDiscoveryApi;

        match method {
            "get_peer_count" => {
                let peer_discovery = self.container.peer_discovery.read();
                let stats = peer_discovery.get_stats();
                Ok(serde_json::json!(format!("0x{:x}", stats.total_peers)))
            }
            "get_peers" => {
                let peer_discovery = self.container.peer_discovery.read();
                let peers = peer_discovery.get_random_peers(100);
                // Convert peers to JSON-serializable format
                let peers_json: Vec<serde_json::Value> = peers
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "id": format!("{:?}", p.node_id),
                            "last_seen": p.last_seen.as_secs()
                        })
                    })
                    .collect();
                Ok(serde_json::json!(peers_json))
            }
            "get_node_info" | "net_listening" => {
                // Node is always listening when running
                // get_node_info is the method name from qc-16 bus_adapter
                Ok(serde_json::json!(true))
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown peer discovery method: {}", method),
            }),
        }
    }

    /// Handle queries for qc-04 State Management.
    async fn handle_state_management_query(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        match method {
            "get_balance" => {
                // Parse address from params
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiQueryError {
                        code: -32602,
                        message: "Missing 'address' parameter".to_string(),
                    })?;

                // For now, return 0 balance (state trie integration needed)
                debug!(address = %address, "Getting balance");
                Ok(serde_json::json!("0x0"))
            }
            "get_code" => {
                // Parse address from params
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiQueryError {
                        code: -32602,
                        message: "Missing 'address' parameter".to_string(),
                    })?;

                // For now, return empty code (no contracts deployed)
                debug!(address = %address, "Getting code");
                Ok(serde_json::json!("0x"))
            }
            "get_transaction_count" => {
                // Parse address from params
                let address = params
                    .get("address")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ApiQueryError {
                        code: -32602,
                        message: "Missing 'address' parameter".to_string(),
                    })?;

                // For now, return 0 nonce (state trie integration needed)
                debug!(address = %address, "Getting transaction count");
                Ok(serde_json::json!("0x0"))
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown state management method: {}", method),
            }),
        }
    }

    /// Handle queries for node-runtime (sync status, node info).
    async fn handle_node_runtime_query(
        &self,
        method: &str,
        _params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        use qc_02_block_storage::BlockStorageApi;

        match method {
            "get_sync_status" => {
                // Check if node is syncing by comparing local height with network
                let storage = self.container.block_storage.read();
                let current_height = storage.get_latest_height().unwrap_or(0);
                
                // For now, we're not syncing if we have genesis block
                // In production, compare with peer heights
                if current_height == 0 {
                    // Still syncing (no blocks yet)
                    Ok(serde_json::json!({
                        "startingBlock": "0x0",
                        "currentBlock": format!("0x{:x}", current_height),
                        "highestBlock": "0x0"
                    }))
                } else {
                    // Not syncing (return false per Ethereum spec)
                    Ok(serde_json::json!(false))
                }
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown node-runtime method: {}", method),
            }),
        }
    }

    /// Handle admin queries for subsystem metrics.
    async fn handle_admin_query(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> Result<serde_json::Value, ApiQueryError> {
        match method {
            "get_subsystem_metrics" => {
                // Params comes from RequestPayload tagged enum: { "type": "...", "data": { "subsystem_id": N } }
                let subsystem_id = params
                    .get("data")
                    .and_then(|d| d.get("subsystem_id"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8;

                self.get_subsystem_specific_metrics(subsystem_id).await
            }
            _ => Err(ApiQueryError {
                code: -32601,
                message: format!("Unknown admin method: {}", method),
            }),
        }
    }

    /// Get subsystem-specific metrics based on ID.
    async fn get_subsystem_specific_metrics(
        &self,
        subsystem_id: u8,
    ) -> Result<serde_json::Value, ApiQueryError> {
        use qc_01_peer_discovery::PeerDiscoveryApi;
        use qc_02_block_storage::BlockStorageApi;

        match subsystem_id {
            // qc-01: Peer Discovery
            1 => {
                let peer_discovery = self.container.peer_discovery.read();
                let stats = peer_discovery.get_stats();
                Ok(serde_json::json!({
                    "peers_connected": stats.total_peers,
                    "routing_table_size": stats.buckets_used * 20, // Approx
                    "buckets_used": stats.buckets_used,
                    "banned_count": stats.banned_count,
                    "pending_verification": stats.pending_verification_count,
                    "max_pending": stats.max_pending_peers,
                    "oldest_peer_age_secs": stats.oldest_peer_age_seconds
                }))
            }
            // qc-02: Block Storage
            2 => {
                let storage = self.container.block_storage.read();
                let latest_height = storage.get_latest_height().unwrap_or(0);
                let finalized_height = storage.get_finalized_height().unwrap_or(0);
                let metadata = storage.get_metadata().unwrap_or_default();
                
                // Disk metrics would come from filesystem adapter in production
                // For now, use placeholder values
                let disk_used_bytes: u64 = 0;
                let disk_capacity_bytes: u64 = 500 * 1024 * 1024 * 1024; // 500GB
                
                Ok(serde_json::json!({
                    "latest_height": latest_height,
                    "finalized_height": finalized_height,
                    "total_blocks": metadata.total_blocks,
                    "genesis_hash": metadata.genesis_hash.map(|h| hex::encode(h)),
                    "storage_version": metadata.storage_version,
                    "disk_used_bytes": disk_used_bytes,
                    "disk_capacity_bytes": disk_capacity_bytes,
                    "pending_assemblies": 0, // Would need assembler state from runtime adapter
                    "assembly_timeout_secs": 30
                }))
            }
            // qc-03: Transaction Indexing
            3 => {
                // Get stats from transaction indexing service
                // For now, derive from block storage height
                let storage = self.container.block_storage.read();
                let height = storage.get_latest_height().unwrap_or(0);
                
                Ok(serde_json::json!({
                    "total_indexed": height, // Each block has transactions indexed
                    "cached_trees": 0, // Would come from TransactionIndex.stats()
                    "max_cached_trees": 1000,
                    "proofs_generated": 0, // Would track in metrics
                    "proofs_verified": 0,
                    "last_merkle_root": null
                }))
            }
            // qc-04: State Management
            4 => {
                // Get stats from state management service
                // For now, return placeholder structure matching panel expectations
                Ok(serde_json::json!({
                    "total_accounts": 0, // Would come from trie.account_count()
                    "total_contracts": 0, // Accounts with non-empty code_hash
                    "current_state_root": null, // hex encoded current root
                    "cache_size_mb": 512,
                    "proofs_generated": 0,
                    "snapshots_count": 0,
                    "pruning_depth": 1000,
                    "trie_depth": 0, // Current max depth in trie
                    "trie_nodes": 0, // Total nodes in trie
                    "storage_slots": 0 // Total storage slots across all contracts
                }))
            }
            // qc-05: Block Propagation
            5 => {
                Ok(serde_json::json!({
                    "blocks_relayed": 0, // Would track in metrics
                    "compact_blocks_percent": 0.0
                }))
            }
            // qc-06: Mempool
            6 => {
                let pool = self.container.mempool.read();
                // Use current timestamp for status
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let status = pool.status(now);
                Ok(serde_json::json!({
                    "pending_txs": status.pending_count,
                    "pending_inclusion": status.pending_inclusion_count,
                    "total_gas": status.total_gas,
                    "memory_bytes": status.memory_bytes,
                    "oldest_tx_age_ms": status.oldest_tx_age_ms
                }))
            }
            // qc-08: Consensus
            8 => {
                Ok(serde_json::json!({
                    "mode": "PoS",
                    "validators": 0, // Would query validator set
                    "current_round": 0,
                    "attestations": 0
                }))
            }
            // qc-09: Finality
            9 => {
                use qc_09_finality::FinalityApi;
                let last_finalized = self.container.finality.get_last_finalized().await;
                let depth = self.container.finality.get_finality_lag().await;
                Ok(serde_json::json!({
                    "last_finalized_epoch": last_finalized.as_ref().map(|c| c.epoch).unwrap_or(0),
                    "last_finalized_block": last_finalized.as_ref().map(|c| c.block_height).unwrap_or(0),
                    "finality_depth": depth,
                    "circuit_breaker": "ok"
                }))
            }
            // qc-10: Signature Verification
            10 => {
                Ok(serde_json::json!({
                    "verifications_total": 0, // Would track in metrics
                    "batch_verify_enabled": true,
                    "cache_size": 10000
                }))
            }
            // qc-16: API Gateway (self)
            16 => {
                Ok(serde_json::json!({
                    "requests_total": 0, // Would track in gateway metrics
                    "websocket_connections": 0
                }))
            }
            // Unimplemented subsystems
            7 | 11 | 12 | 13 | 14 | 15 => {
                Ok(serde_json::json!({
                    "implemented": false
                }))
            }
            _ => Err(ApiQueryError {
                code: -32602,
                message: format!("Unknown subsystem ID: {}", subsystem_id),
            }),
        }
    }

    /// Convert target string to subsystem ID.
    fn target_to_subsystem_id(target: &str) -> u8 {
        match target {
            "qc-01-peer-discovery" => 1,
            "qc-02-block-storage" => 2,
            "qc-03-transaction-indexing" => 3,
            "qc-04-state-management" => 4,
            "qc-05-block-propagation" => 5,
            "qc-06-mempool" => 6,
            "qc-08-consensus" => 8,
            "qc-09-finality" => 9,
            "qc-10-signature-verification" => 10,
            "qc-16-api-gateway" => 16,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_to_subsystem_id() {
        assert_eq!(ApiQueryHandler::target_to_subsystem_id("qc-02-block-storage"), 2);
        assert_eq!(ApiQueryHandler::target_to_subsystem_id("qc-06-mempool"), 6);
        assert_eq!(ApiQueryHandler::target_to_subsystem_id("qc-01-peer-discovery"), 1);
        assert_eq!(ApiQueryHandler::target_to_subsystem_id("unknown"), 0);
    }
}
