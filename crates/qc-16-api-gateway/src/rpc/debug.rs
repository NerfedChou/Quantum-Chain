//! Debug JSON-RPC methods per SPEC-16 Section 3.3 (Admin tier).

use crate::domain::error::{ApiError, ApiResult};
use crate::domain::types::{BlockId, Hash};
use crate::ipc::handler::IpcHandler;
use std::sync::Arc;
use tracing::instrument;

/// Debug RPC methods handler
///
/// ADMIN TIER ONLY - These methods expose internal state and can be resource-intensive.
pub struct DebugRpc {
    #[allow(dead_code)]
    ipc: Arc<IpcHandler>,
}

impl DebugRpc {
    pub fn new(ipc: Arc<IpcHandler>) -> Self {
        Self { ipc }
    }

    /// debug_traceTransaction - Trace transaction execution
    #[instrument(skip(self))]
    pub async fn trace_transaction(
        &self,
        hash: Hash,
        options: Option<TraceOptions>,
    ) -> ApiResult<serde_json::Value> {
        // This would require deep integration with EVM execution
        // For now, return a placeholder structure
        Err(ApiError::method_not_supported(
            "debug_traceTransaction requires EVM tracing integration",
        ))
    }

    /// debug_traceBlockByHash - Trace all transactions in block
    #[instrument(skip(self))]
    pub async fn trace_block_by_hash(
        &self,
        hash: Hash,
        options: Option<TraceOptions>,
    ) -> ApiResult<Vec<serde_json::Value>> {
        Err(ApiError::method_not_supported(
            "debug_traceBlockByHash requires EVM tracing integration",
        ))
    }

    /// debug_traceBlockByNumber - Trace all transactions in block
    #[instrument(skip(self))]
    pub async fn trace_block_by_number(
        &self,
        block_id: BlockId,
        options: Option<TraceOptions>,
    ) -> ApiResult<Vec<serde_json::Value>> {
        Err(ApiError::method_not_supported(
            "debug_traceBlockByNumber requires EVM tracing integration",
        ))
    }

    /// debug_traceCall - Trace call without transaction
    #[instrument(skip(self))]
    pub async fn trace_call(
        &self,
        call: serde_json::Value,
        block_id: BlockId,
        options: Option<TraceOptions>,
    ) -> ApiResult<serde_json::Value> {
        Err(ApiError::method_not_supported(
            "debug_traceCall requires EVM tracing integration",
        ))
    }

    /// debug_getRawBlock - Returns raw block bytes
    #[instrument(skip(self))]
    pub async fn get_raw_block(&self, block_id: BlockId) -> ApiResult<String> {
        // Would query block storage for RLP-encoded block
        Err(ApiError::method_not_supported(
            "debug_getRawBlock not implemented",
        ))
    }

    /// debug_getRawHeader - Returns raw header bytes
    #[instrument(skip(self))]
    pub async fn get_raw_header(&self, block_id: BlockId) -> ApiResult<String> {
        Err(ApiError::method_not_supported(
            "debug_getRawHeader not implemented",
        ))
    }

    /// debug_getRawTransaction - Returns raw transaction bytes
    #[instrument(skip(self))]
    pub async fn get_raw_transaction(&self, hash: Hash) -> ApiResult<String> {
        Err(ApiError::method_not_supported(
            "debug_getRawTransaction not implemented",
        ))
    }

    /// debug_getRawReceipts - Returns raw receipts for block
    #[instrument(skip(self))]
    pub async fn get_raw_receipts(&self, block_id: BlockId) -> ApiResult<Vec<String>> {
        Err(ApiError::method_not_supported(
            "debug_getRawReceipts not implemented",
        ))
    }

    /// debug_setHead - Set chain head (DANGEROUS)
    #[instrument(skip(self))]
    pub async fn set_head(&self, block_number: u64) -> ApiResult<bool> {
        Err(ApiError::method_not_supported(
            "debug_setHead is disabled for safety",
        ))
    }

    /// debug_storageRangeAt - Returns storage range at block
    #[instrument(skip(self))]
    pub async fn storage_range_at(
        &self,
        block_hash: Hash,
        tx_index: u64,
        contract_address: Hash,
        key_start: Hash,
        max_result: u64,
    ) -> ApiResult<serde_json::Value> {
        Err(ApiError::method_not_supported(
            "debug_storageRangeAt not implemented",
        ))
    }
}

/// Trace options for debug methods
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceOptions {
    /// Tracer to use (callTracer, prestateTracer, etc.)
    #[serde(default)]
    pub tracer: Option<String>,
    /// Tracer configuration
    #[serde(default)]
    pub tracer_config: Option<serde_json::Value>,
    /// Timeout for trace operation
    #[serde(default)]
    pub timeout: Option<String>,
    /// Disable storage capture
    #[serde(default)]
    pub disable_storage: bool,
    /// Disable stack capture
    #[serde(default)]
    pub disable_stack: bool,
    /// Enable memory capture
    #[serde(default)]
    pub enable_memory: bool,
    /// Enable return data capture
    #[serde(default)]
    pub enable_return_data: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_options_default() {
        let opts = TraceOptions::default();
        assert!(opts.tracer.is_none());
        assert!(!opts.disable_storage);
    }
}
