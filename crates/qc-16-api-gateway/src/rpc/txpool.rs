//! TxPool JSON-RPC methods per SPEC-16 Section 3.2 (Protected tier).

use crate::domain::types::Address;
use crate::ipc::handler::IpcHandler;
use crate::ipc::requests::*;
use crate::{ApiError, ApiResult};
use std::sync::Arc;
use tracing::instrument;

/// TxPool RPC methods handler
pub struct TxPoolRpc {
    ipc: Arc<IpcHandler>,
}

impl TxPoolRpc {
    pub fn new(ipc: Arc<IpcHandler>) -> Self {
        Self { ipc }
    }

    /// txpool_status - Returns txpool status (pending/queued counts)
    #[instrument(skip(self))]
    pub async fn status(&self) -> ApiResult<serde_json::Value> {
        let result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::GetTxPoolStatus(GetTxPoolStatusRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result)
    }

    /// txpool_content - Returns full txpool content
    #[instrument(skip(self))]
    pub async fn content(&self) -> ApiResult<serde_json::Value> {
        let result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::GetTxPoolContent(GetTxPoolContentRequest { address: None }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result)
    }

    /// txpool_contentFrom - Returns txpool content for specific address
    #[instrument(skip(self))]
    pub async fn content_from(&self, address: Address) -> ApiResult<serde_json::Value> {
        let result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::GetTxPoolContent(GetTxPoolContentRequest {
                    address: Some(address),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        Ok(result)
    }

    /// txpool_inspect - Returns txpool summary (textual representation)
    #[instrument(skip(self))]
    pub async fn inspect(&self) -> ApiResult<serde_json::Value> {
        // Get full content and format as text summaries
        let content = self.content().await?;

        // Transform to textual format:
        // "pending": { "0x...": { "0": "0x... → 0x...: 1 wei + 21000 gas × 1 gwei" } }
        let mut result = serde_json::json!({
            "pending": {},
            "queued": {}
        });

        // Transform pending
        if let Some(pending) = content.get("pending").and_then(|p| p.as_object()) {
            let mut pending_map = serde_json::Map::new();
            for (addr, nonces) in pending {
                if let Some(nonces_obj) = nonces.as_object() {
                    let mut nonce_map = serde_json::Map::new();
                    for (nonce, tx) in nonces_obj {
                        let summary = format_tx_summary(tx);
                        nonce_map.insert(nonce.clone(), serde_json::Value::String(summary));
                    }
                    pending_map.insert(addr.clone(), serde_json::Value::Object(nonce_map));
                }
            }
            result["pending"] = serde_json::Value::Object(pending_map);
        }

        // Transform queued
        if let Some(queued) = content.get("queued").and_then(|q| q.as_object()) {
            let mut queued_map = serde_json::Map::new();
            for (addr, nonces) in queued {
                if let Some(nonces_obj) = nonces.as_object() {
                    let mut nonce_map = serde_json::Map::new();
                    for (nonce, tx) in nonces_obj {
                        let summary = format_tx_summary(tx);
                        nonce_map.insert(nonce.clone(), serde_json::Value::String(summary));
                    }
                    queued_map.insert(addr.clone(), serde_json::Value::Object(nonce_map));
                }
            }
            result["queued"] = serde_json::Value::Object(queued_map);
        }

        Ok(result)
    }
}

/// Format transaction as text summary for txpool_inspect
fn format_tx_summary(tx: &serde_json::Value) -> String {
    let from = tx.get("from").and_then(|v| v.as_str()).unwrap_or("0x0");
    let to = tx
        .get("to")
        .and_then(|v| v.as_str())
        .unwrap_or("contract creation");
    let value = tx.get("value").and_then(|v| v.as_str()).unwrap_or("0x0");
    let gas = tx.get("gas").and_then(|v| v.as_u64()).unwrap_or(21000);
    let gas_price = tx.get("gasPrice").and_then(|v| v.as_str()).unwrap_or("0x0");

    format!("{} → {}: {} + {} gas × {}", from, to, value, gas, gas_price)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tx_summary() {
        let tx = serde_json::json!({
            "from": "0xabc",
            "to": "0xdef",
            "value": "0x1000",
            "gas": 21000,
            "gasPrice": "0x3b9aca00"
        });

        let summary = format_tx_summary(&tx);
        assert!(summary.contains("0xabc"));
        assert!(summary.contains("0xdef"));
        assert!(summary.contains("21000"));
    }
}
