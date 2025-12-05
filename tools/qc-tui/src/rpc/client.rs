//! JSON-RPC client for qc-16 API Gateway communication.

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a, T: Serialize> {
    jsonrpc: &'static str,
    method: &'a str,
    params: T,
    id: u64,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// RPC client for communicating with qc-16 API Gateway.
pub struct RpcClient {
    http_client: reqwest::Client,
    rpc_url: String,
    request_id: AtomicU64,
}

/// Error type for RPC client creation (reserved for future use).
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum RpcClientError {
    #[error("Failed to create HTTP client: {0}")]
    HttpClientCreation(#[from] reqwest::Error),
}

impl RpcClient {
    /// Create a new RPC client.
    pub fn new(rpc_url: String) -> Self {
        // Use default client if builder fails - reqwest::Client::new() is infallible
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            rpc_url,
            request_id: AtomicU64::new(1),
        }
    }
    
    /// Create a new RPC client with custom timeout (reserved for future use).
    #[allow(dead_code)]
    pub fn with_timeout(rpc_url: String, timeout_secs: u64) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            http_client,
            rpc_url,
            request_id: AtomicU64::new(1),
        }
    }

    /// Make a JSON-RPC call.
    async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id,
        };

        let response = self
            .http_client
            .post(&self.rpc_url)
            .json(&request)
            .send()
            .await
            .context("Failed to send RPC request")?;

        let rpc_response: JsonRpcResponse<R> = response
            .json()
            .await
            .context("Failed to parse RPC response")?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("RPC error {}: {}", error.code, error.message);
        }

        rpc_response
            .result
            .ok_or_else(|| anyhow::anyhow!("RPC response missing result"))
    }

    /// eth_blockNumber - Get current block height.
    pub async fn get_block_number(&self) -> Result<u64> {
        let result: String = self.call("eth_blockNumber", Vec::<()>::new()).await?;
        parse_hex_u64(&result)
    }

    /// eth_chainId - Get chain ID.
    pub async fn get_chain_id(&self) -> Result<u64> {
        let result: String = self.call("eth_chainId", Vec::<()>::new()).await?;
        parse_hex_u64(&result)
    }

    /// eth_gasPrice - Get current gas price in wei.
    pub async fn get_gas_price(&self) -> Result<u64> {
        let result: String = self.call("eth_gasPrice", Vec::<()>::new()).await?;
        parse_hex_u64(&result)
    }

    /// eth_syncing - Get sync status.
    pub async fn get_syncing(&self) -> Result<SyncStatus> {
        let result: serde_json::Value = self.call("eth_syncing", Vec::<()>::new()).await?;

        if result.is_boolean() && !result.as_bool().unwrap_or(true) {
            Ok(SyncStatus::Synced)
        } else if let Some(obj) = result.as_object() {
            let current = obj
                .get("currentBlock")
                .and_then(|v| v.as_str())
                .map(|s| parse_hex_u64(s).unwrap_or(0))
                .unwrap_or(0);
            let highest = obj
                .get("highestBlock")
                .and_then(|v| v.as_str())
                .map(|s| parse_hex_u64(s).unwrap_or(0))
                .unwrap_or(0);

            Ok(SyncStatus::Syncing { current, highest })
        } else {
            Ok(SyncStatus::Synced)
        }
    }

    /// net_peerCount - Get connected peer count.
    pub async fn get_peer_count(&self) -> Result<u64> {
        let result: String = self.call("net_peerCount", Vec::<()>::new()).await?;
        parse_hex_u64(&result)
    }

    /// net_listening - Check if node is listening.
    pub async fn get_listening(&self) -> Result<bool> {
        self.call("net_listening", Vec::<()>::new()).await
    }

    /// net_version - Get network ID.
    pub async fn get_network_version(&self) -> Result<String> {
        self.call("net_version", Vec::<()>::new()).await
    }

    /// txpool_status - Get mempool status.
    pub async fn get_txpool_status(&self) -> Result<TxPoolStatus> {
        let result: serde_json::Value = self.call("txpool_status", Vec::<()>::new()).await?;

        let pending = result
            .get("pending")
            .and_then(|v| v.as_str())
            .map(|s| parse_hex_u64(s).unwrap_or(0))
            .unwrap_or(0);

        let queued = result
            .get("queued")
            .and_then(|v| v.as_str())
            .map(|s| parse_hex_u64(s).unwrap_or(0))
            .unwrap_or(0);

        Ok(TxPoolStatus { pending, queued })
    }

    /// txpool_content - Get mempool content.
    pub async fn get_txpool_content(&self) -> Result<TxPoolContent> {
        let result: serde_json::Value = self.call("txpool_content", Vec::<()>::new()).await?;

        let pending = Self::parse_txpool_section(result.get("pending"));
        let queued = Self::parse_txpool_section(result.get("queued"));

        Ok(TxPoolContent { pending, queued })
    }
    
    /// Parse a txpool section (pending or queued) from JSON.
    fn parse_txpool_section(section: Option<&serde_json::Value>) -> Vec<TxInfo> {
        let Some(obj) = section.and_then(|v| v.as_object()) else {
            return Vec::new();
        };
        
        obj.values()
            .filter_map(|nonces| nonces.as_object())
            .flat_map(|nonces_obj| nonces_obj.values())
            .filter_map(|tx| serde_json::from_value::<TxInfo>(tx.clone()).ok())
            .collect()
    }

    /// eth_getBlockByNumber - Get block by number.
    pub async fn get_block_by_number(&self, block_num: u64, full_txs: bool) -> Result<Option<BlockInfo>> {
        let block_hex = format!("0x{:x}", block_num);
        let result: serde_json::Value = self.call("eth_getBlockByNumber", (block_hex, full_txs)).await?;

        if result.is_null() {
            return Ok(None);
        }

        serde_json::from_value(result).map(Some).context("Failed to parse block")
    }

    /// admin_peers - Get connected peer info (admin API, localhost only).
    pub async fn get_admin_peers(&self) -> Result<Vec<PeerInfo>> {
        let result: serde_json::Value = self.call("admin_peers", Vec::<()>::new()).await?;

        if let Some(peers) = result.as_array() {
            let mut peer_list = Vec::new();
            for peer in peers {
                if let Ok(info) = serde_json::from_value::<PeerInfo>(peer.clone()) {
                    peer_list.push(info);
                }
            }
            Ok(peer_list)
        } else {
            Ok(Vec::new())
        }
    }

    /// admin_nodeInfo - Get node info (admin API).
    pub async fn get_node_info(&self) -> Result<NodeInfo> {
        self.call("admin_nodeInfo", Vec::<()>::new()).await
    }
}

/// Transaction pool status.
#[derive(Debug, Clone, Default)]
pub struct TxPoolStatus {
    pub pending: u64,
    pub queued: u64,
}

/// Transaction pool content.
#[derive(Debug, Clone, Default)]
pub struct TxPoolContent {
    pub pending: Vec<TxInfo>,
    pub queued: Vec<TxInfo>,
}

/// Transaction info from txpool.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct TxInfo {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    pub gas_price: Option<String>,
    pub nonce: String,
}

impl TxInfo {
    /// Get short hash.
    pub fn short_hash(&self) -> String {
        if self.hash.len() > 12 {
            format!("{}...", &self.hash[..12])
        } else {
            self.hash.clone()
        }
    }

    /// Get short from address.
    pub fn short_from(&self) -> String {
        if self.from.len() > 10 {
            format!("{}...", &self.from[..10])
        } else {
            self.from.clone()
        }
    }

    /// Get short to address.
    pub fn short_to(&self) -> String {
        self.to.as_ref().map(|t| {
            if t.len() > 10 {
                format!("{}...", &t[..10])
            } else {
                t.clone()
            }
        }).unwrap_or_else(|| "Contract".to_string())
    }

    /// Get value in ETH.
    pub fn value_eth(&self) -> f64 {
        parse_hex_u64(&self.value).unwrap_or(0) as f64 / 1e18
    }

    /// Get gas price in gwei.
    pub fn gas_price_gwei(&self) -> f64 {
        self.gas_price.as_ref()
            .and_then(|p| parse_hex_u64(p).ok())
            .unwrap_or(0) as f64 / 1e9
    }

    /// Get nonce as u64.
    pub fn nonce_u64(&self) -> u64 {
        parse_hex_u64(&self.nonce).unwrap_or(0)
    }
}

/// Block info from eth_getBlockByNumber.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BlockInfo {
    pub number: String,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: String,
    pub gas_used: String,
    pub gas_limit: String,
    #[serde(default)]
    pub transactions: serde_json::Value,
}

impl BlockInfo {
    /// Get block number as u64.
    pub fn block_number(&self) -> u64 {
        parse_hex_u64(&self.number).unwrap_or(0)
    }

    /// Get short hash.
    pub fn short_hash(&self) -> String {
        if self.hash.len() > 12 {
            format!("{}...", &self.hash[..12])
        } else {
            self.hash.clone()
        }
    }

    /// Get transaction count.
    pub fn tx_count(&self) -> usize {
        self.transactions.as_array().map(|a| a.len()).unwrap_or(0)
    }

    /// Get gas used as u64.
    pub fn gas_used_u64(&self) -> u64 {
        parse_hex_u64(&self.gas_used).unwrap_or(0)
    }

    /// Get timestamp as u64 (reserved for future use).
    #[allow(dead_code)]
    pub fn timestamp_u64(&self) -> u64 {
        parse_hex_u64(&self.timestamp).unwrap_or(0)
    }
}

/// Peer info from admin_peers.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PeerInfo {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub caps: Vec<String>,
    #[serde(default)]
    pub network: PeerNetwork,
    #[serde(default)]
    pub protocols: serde_json::Value,
}

/// Peer network info.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PeerNetwork {
    #[serde(default)]
    pub local_address: String,
    #[serde(default)]
    pub remote_address: String,
    #[serde(default)]
    pub inbound: bool,
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub static_node: bool,
}

impl PeerInfo {
    /// Get short peer ID.
    pub fn short_id(&self) -> String {
        if self.id.len() > 16 {
            format!("{}...", &self.id[..16])
        } else {
            self.id.clone()
        }
    }

    /// Get peer name or "Unknown".
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            "Unknown"
        } else {
            &self.name
        }
    }

    /// Get remote address.
    pub fn remote_addr(&self) -> &str {
        if self.network.remote_address.is_empty() {
            "N/A"
        } else {
            &self.network.remote_address
        }
    }

    /// Get direction string.
    pub fn direction(&self) -> &'static str {
        if self.network.inbound {
            "Inbound"
        } else {
            "Outbound"
        }
    }

    /// Check if trusted.
    pub fn is_trusted(&self) -> bool {
        self.network.trusted
    }
}

/// Node info from admin_nodeInfo.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct NodeInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub enode: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub ports: NodePorts,
    #[serde(default)]
    pub listen_addr: String,
    #[serde(default)]
    pub protocols: serde_json::Value,
}

/// Node ports.
#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct NodePorts {
    #[serde(default)]
    pub discovery: u16,
    #[serde(default)]
    pub listener: u16,
}

/// Sync status enum.
#[derive(Debug, Clone)]
pub enum SyncStatus {
    Synced,
    Syncing { current: u64, highest: u64 },
}

impl SyncStatus {
    /// Get sync percentage (0-100).
    pub fn percentage(&self) -> u8 {
        match self {
            SyncStatus::Synced => 100,
            SyncStatus::Syncing { current, highest } => {
                if *highest == 0 {
                    0
                } else {
                    ((current * 100) / highest).min(100) as u8
                }
            }
        }
    }

    /// Check if fully synced.
    pub fn is_synced(&self) -> bool {
        matches!(self, SyncStatus::Synced)
    }
}

/// Parse hex string to u64.
fn parse_hex_u64(s: &str) -> Result<u64> {
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).context("Failed to parse hex number")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex_u64("0x0").unwrap(), 0);
        assert_eq!(parse_hex_u64("0x1").unwrap(), 1);
        assert_eq!(parse_hex_u64("0xff").unwrap(), 255);
        assert_eq!(parse_hex_u64("0x12d687").unwrap(), 1234567);
    }

    #[test]
    fn test_sync_status_percentage() {
        assert_eq!(SyncStatus::Synced.percentage(), 100);
        assert_eq!(
            SyncStatus::Syncing {
                current: 50,
                highest: 100
            }
            .percentage(),
            50
        );
        assert_eq!(
            SyncStatus::Syncing {
                current: 0,
                highest: 100
            }
            .percentage(),
            0
        );
    }
}
