use crate::domain::error::ApiError;
use crate::middleware::GatewayMetrics;
use crate::rpc::RpcHandlers;
use std::sync::Arc;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub rpc_handlers: Arc<RpcHandlers>,
    pub metrics: Arc<GatewayMetrics>,
}

/// Route JSON-RPC method to appropriate handler.
///
/// Per SPEC-16 Section 3, methods are organized by tier:
/// - Tier 1 (Public): eth_*, web3_*, net_version
/// - Tier 2 (Protected): txpool_*, net_peerCount, net_listening
/// - Tier 3 (Admin): admin_*, miner_*, debug_*
pub async fn route_method(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    
    match method {
        // Chain Info
        "eth_chainId" | "eth_blockNumber" | "eth_gasPrice" | "eth_syncing" => {
            route_eth_chain(state, method).await
        }

        // Account State
        "eth_accounts" | "eth_getBalance" | "eth_getCode" | "eth_getStorageAt" | "eth_getTransactionCount" => {
            route_eth_account(state, method, params).await
        }

        // Block Data
        "eth_getBlockByHash" | "eth_getBlockByNumber" | "eth_getBlockTransactionCountByHash" | 
        "eth_getBlockTransactionCountByNumber" | "eth_getUncleCountByBlockHash" | "eth_getUncleCountByBlockNumber" => {
            route_eth_block(state, method, params).await
        }

        // Transaction Data
        "eth_getTransactionByHash" | "eth_getTransactionReceipt" | "eth_getBlockReceipts" | "eth_sendRawTransaction" => {
            route_eth_transaction(state, method, params).await
        }

        // Execution & Logs
        "eth_call" | "eth_estimateGas" | "eth_getLogs" => {
            route_eth_execution(state, method, params).await
        }

        // Fee Market
        "eth_maxPriorityFeePerGas" | "eth_feeHistory" => {
            route_eth_fee_market(state, method, params).await
        }

        "web3_clientVersion" | "web3_sha3" => {
            route_web3_namespace(state, method, params).await
        }

        "net_version" | "net_listening" | "net_peerCount" => {
            route_net_namespace(state, method, params).await
        }

        "txpool_status" | "txpool_content" | "txpool_inspect" | "txpool_contentFrom" => {
            route_txpool_namespace(state, method, params).await
        }

        "admin_peers" | "admin_nodeInfo" | "admin_addPeer" | "admin_removePeer" | "admin_datadir" => {
            route_admin_namespace(state, method, params).await
        }
        
        "debug_traceBlockByNumber" | "debug_subsystemStatus" => {
            route_debug_namespace(state, method, params).await
        }

        _ => Err(ApiError {
            code: -32601,
            message: format!("Method not found: {}", method),
            data: None,
        }),
    }
}

async fn route_eth_chain(
    state: &AppState,
    method: &str,
) -> Result<serde_json::Value, ApiError> {
    match method {
        "eth_chainId" => state.rpc_handlers.eth.chain_id().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        "eth_blockNumber" => state.rpc_handlers.eth.block_number().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        "eth_gasPrice" => state.rpc_handlers.eth.gas_price().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        "eth_syncing" => state.rpc_handlers.eth.syncing().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_eth_account(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{Address, BlockId, U256};
    
    match method {
        "eth_accounts" => state.rpc_handlers.eth.accounts().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        "eth_getBalance" => {
            let address: Address = parse_param(params, 0)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 1);
            state.rpc_handlers.eth.get_balance(address, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_getCode" => {
            let address: Address = parse_param(params, 0)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 1);
            state.rpc_handlers.eth.get_code(address, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_getStorageAt" => {
            let address: Address = parse_param(params, 0)?;
            let position: U256 = parse_param(params, 1)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 2);
            state.rpc_handlers.eth.get_storage_at(address, position, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_getTransactionCount" => {
            let address: Address = parse_param(params, 0)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 1);
            state.rpc_handlers.eth.get_transaction_count(address, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_eth_block(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{BlockId, Hash};

    match method {
        "eth_getBlockByHash" => {
            let hash: Hash = parse_param(params, 0)?;
            let full_tx: bool = parse_param_optional(params, 1).unwrap_or(false);
            state.rpc_handlers.eth.get_block_by_hash(hash, full_tx).await.map(|v| v.unwrap_or(serde_json::Value::Null))
        }
        "eth_getBlockByNumber" => {
            let block_id: BlockId = parse_param(params, 0)?;
            let full_tx: bool = parse_param_optional(params, 1).unwrap_or(false);
            state.rpc_handlers.eth.get_block_by_number(block_id, full_tx).await.map(|v| v.unwrap_or(serde_json::Value::Null))
        }
        "eth_getBlockTransactionCountByHash" => {
            let hash: Hash = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_block_transaction_count_by_hash(hash).await.map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        "eth_getBlockTransactionCountByNumber" => {
            let block_id: BlockId = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_block_transaction_count_by_number(block_id).await.map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        "eth_getUncleCountByBlockHash" => {
            let hash: Hash = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_uncle_count_by_block_hash(hash).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_getUncleCountByBlockNumber" => {
            let block_id: BlockId = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_uncle_count_by_block_number(block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_eth_transaction(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{BlockId, Hash};

    match method {
        "eth_getTransactionByHash" => {
            let hash: Hash = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_transaction_by_hash(hash).await.map(|v| v.unwrap_or(serde_json::Value::Null))
        }
        "eth_getTransactionReceipt" => {
            let hash: Hash = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_transaction_receipt(hash).await.map(|v| v.unwrap_or(serde_json::Value::Null))
        }
        "eth_getBlockReceipts" => {
            let block_id: BlockId = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_block_receipts(block_id).await.map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        "eth_sendRawTransaction" => {
            let raw_tx: crate::domain::types::Bytes = parse_param(params, 0)?;
            state.rpc_handlers.eth.send_raw_transaction(raw_tx).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_eth_execution(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{BlockId, CallRequest, Filter};

    match method {
         "eth_call" => {
            let call: CallRequest = parse_param(params, 0)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 1);
            state.rpc_handlers.eth.call(call, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_estimateGas" => {
            let call: CallRequest = parse_param(params, 0)?;
            let block_id: Option<BlockId> = parse_param_optional(params, 1);
            state.rpc_handlers.eth.estimate_gas(call, block_id).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "eth_getLogs" => {
            let filter: Filter = parse_param(params, 0)?;
            state.rpc_handlers.eth.get_logs(filter).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_eth_fee_market(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{BlockId, U256};

    match method {
        "eth_maxPriorityFeePerGas" => state.rpc_handlers.eth.max_priority_fee_per_gas().await.map(|v| serde_json::to_value(v).unwrap_or_default()),
        "eth_feeHistory" => {
            let block_count: U256 = parse_param(params, 0)?;
            let newest_block: BlockId = parse_param(params, 1)?;
            let percentiles: Option<Vec<f64>> = parse_param_optional(params, 2);
            state.rpc_handlers.eth.fee_history(block_count, newest_block, percentiles).await.map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_web3_namespace(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::Bytes;

    match method {
        "web3_clientVersion" => state
            .rpc_handlers
            .web3
            .client_version()
            .await
            .map(|v| serde_json::json!(v)),

        "web3_sha3" => {
            let data: Bytes = parse_param(params, 0)?;
            state
                .rpc_handlers
                .web3
                .sha3(data)
                .await
                .map(|v| serde_json::json!(v))
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_net_namespace(
    state: &AppState,
    method: &str,
    _params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    match method {
        "net_version" => state
            .rpc_handlers
            .net
            .version()
            .await
            .map(|v| serde_json::json!(v)),

        "net_listening" => state
            .rpc_handlers
            .net
            .listening()
            .await
            .map(|v| serde_json::json!(v)),

        "net_peerCount" => state
            .rpc_handlers
            .net
            .peer_count()
            .await
            .map(|v| serde_json::json!(v)),

        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_txpool_namespace(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::Address;

    match method {
        "txpool_status" => state.rpc_handlers.txpool.status().await,
        "txpool_content" => state.rpc_handlers.txpool.content().await,
        "txpool_inspect" => state.rpc_handlers.txpool.inspect().await,
        "txpool_contentFrom" => {
            let address: Address = parse_param(params, 0)?;
            state.rpc_handlers.txpool.content_from(address).await
        }
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_admin_namespace(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    match method {
        "admin_peers" => state.rpc_handlers.admin.peers().await,
        "admin_nodeInfo" => state.rpc_handlers.admin.node_info().await,
        "admin_addPeer" => {
            let enode: String = parse_param(params, 0)?;
            state
                .rpc_handlers
                .admin
                .add_peer(enode)
                .await
                .map(|v| serde_json::json!(v))
        }
        "admin_removePeer" => {
            let enode: String = parse_param(params, 0)?;
            state
                .rpc_handlers
                .admin
                .remove_peer(enode)
                .await
                .map(|v| serde_json::json!(v))
        }
        "admin_datadir" => state
            .rpc_handlers
            .admin
            .datadir()
            .await
            .map(|v| serde_json::json!(v)),
        _ => unreachable!("Filtered by caller"),
    }
}

async fn route_debug_namespace(
    state: &AppState,
    method: &str,
    params: Option<&serde_json::Value>,
) -> Result<serde_json::Value, ApiError> {
    use crate::domain::types::{BlockId};
    use crate::rpc::debug::TraceOptions;

    match method {
        "debug_traceBlockByNumber" => {
            let block_id: BlockId = parse_param(params, 0)?;
            let options: Option<TraceOptions> = parse_param_optional(params, 1);
            state
                .rpc_handlers
                .debug
                .trace_block_by_number(block_id, options)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        "debug_subsystemStatus" => {
            let subsystem_id: String = parse_param(params, 0)?;
            state
                .rpc_handlers
                .debug
                .subsystem_status(subsystem_id)
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default())
        }
        _ => unreachable!("Filtered by caller"),
    }
}

/// Parse a required parameter from JSON-RPC params array.
fn parse_param<T: serde::de::DeserializeOwned>(
    params: Option<&serde_json::Value>,
    index: usize,
) -> Result<T, ApiError> {
    let param = params
        .and_then(|p| {
            if p.is_array() {
                p.get(index)
            } else if index == 0 {
                Some(p)
            } else {
                None
            }
        })
        .ok_or_else(|| ApiError {
            code: -32602,
            message: format!("Missing parameter at index {}", index),
            data: None,
        })?;

    serde_json::from_value(param.clone()).map_err(|e| ApiError {
        code: -32602,
        message: format!("Invalid parameter at index {}: {}", index, e),
        data: None,
    })
}

/// Parse an optional parameter from JSON-RPC params array.
fn parse_param_optional<T: serde::de::DeserializeOwned>(
    params: Option<&serde_json::Value>,
    index: usize,
) -> Option<T> {
    params
        .and_then(|p| {
            if p.is_array() {
                p.get(index)
            } else if index == 0 {
                Some(p)
            } else {
                None
            }
        })
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}
