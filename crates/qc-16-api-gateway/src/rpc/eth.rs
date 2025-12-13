//! Ethereum JSON-RPC methods (eth_*) per SPEC-16 Section 3.1.

use crate::{ApiError, ApiResult};
use crate::domain::types::*;
use crate::ipc::handler::IpcHandler;
use crate::ipc::requests::*;
use crate::ipc::validation::validate_raw_transaction;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Ethereum RPC methods handler
pub struct EthRpc {
    ipc: Arc<IpcHandler>,
    chain_id: u64,
}

impl EthRpc {
    pub fn new(ipc: Arc<IpcHandler>, chain_id: u64) -> Self {
        Self { ipc, chain_id }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // CHAIN INFO
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_chainId - Returns the chain ID
    #[instrument(skip(self))]
    pub async fn chain_id(&self) -> ApiResult<U256> {
        Ok(U256::from(self.chain_id))
    }

    /// eth_blockNumber - Returns current block number
    #[instrument(skip(self))]
    pub async fn block_number(&self) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-02-block-storage",
                RequestPayload::GetBlockNumber(GetBlockNumberRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        // Parse result as hex string or number
        let block_num: u64 = if let Some(s) = result.as_str() {
            u64::from_str_radix(s.trim_start_matches("0x"), 16)
                .map_err(|_| ApiError::internal("Invalid block number format"))?
        } else {
            result
                .as_u64()
                .ok_or_else(|| ApiError::internal("Invalid block number format"))?
        };

        Ok(U256::from(block_num))
    }

    /// eth_gasPrice - Returns current gas price
    #[instrument(skip(self))]
    pub async fn gas_price(&self) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::GetGasPrice(GetGasPriceRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // ACCOUNT STATE
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_getBalance - Returns account balance
    #[instrument(skip(self))]
    pub async fn get_balance(
        &self,
        address: Address,
        block_id: Option<BlockId>,
    ) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-04-state-management",
                RequestPayload::GetBalance(GetBalanceRequest {
                    address,
                    block_id: block_id.unwrap_or_default(),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    /// eth_getCode - Returns contract code
    #[instrument(skip(self))]
    pub async fn get_code(&self, address: Address, block_id: Option<BlockId>) -> ApiResult<Bytes> {
        let result = self
            .ipc
            .request(
                "qc-04-state-management",
                RequestPayload::GetCode(GetCodeRequest {
                    address,
                    block_id: block_id.unwrap_or_default(),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    /// eth_getStorageAt - Returns storage value at position
    #[instrument(skip(self))]
    pub async fn get_storage_at(
        &self,
        address: Address,
        position: U256,
        block_id: Option<BlockId>,
    ) -> ApiResult<Hash> {
        let result = self
            .ipc
            .request(
                "qc-04-state-management",
                RequestPayload::GetStorageAt(GetStorageAtRequest {
                    address,
                    position,
                    block_id: block_id.unwrap_or_default(),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    /// eth_getTransactionCount - Returns account nonce
    #[instrument(skip(self))]
    pub async fn get_transaction_count(
        &self,
        address: Address,
        block_id: Option<BlockId>,
    ) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-04-state-management",
                RequestPayload::GetTransactionCount(GetTransactionCountRequest {
                    address,
                    block_id: block_id.unwrap_or_default(),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        // Parse as hex string
        if let Some(s) = result.as_str() {
            let count = u64::from_str_radix(s.trim_start_matches("0x"), 16)
                .map_err(|_| ApiError::internal("Invalid nonce format"))?;
            Ok(U256::from(count))
        } else {
            serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
        }
    }

    /// eth_accounts - Returns empty list (no managed accounts)
    #[instrument(skip(self))]
    pub async fn accounts(&self) -> ApiResult<Vec<Address>> {
        Ok(Vec::new())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BLOCK DATA
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_getBlockByHash - Returns block by hash
    #[instrument(skip(self))]
    pub async fn get_block_by_hash(
        &self,
        hash: Hash,
        include_transactions: bool,
    ) -> ApiResult<Option<serde_json::Value>> {
        let result = self
            .ipc
            .request(
                "qc-02-block-storage",
                RequestPayload::GetBlockByHash(GetBlockByHashRequest {
                    hash,
                    include_transactions,
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// eth_getBlockByNumber - Returns block by number
    #[instrument(skip(self))]
    pub async fn get_block_by_number(
        &self,
        block_id: BlockId,
        include_transactions: bool,
    ) -> ApiResult<Option<serde_json::Value>> {
        let result = self
            .ipc
            .request(
                "qc-02-block-storage",
                RequestPayload::GetBlockByNumber(GetBlockByNumberRequest {
                    block_id,
                    include_transactions,
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// eth_getBlockTransactionCountByHash
    #[instrument(skip(self))]
    pub async fn get_block_transaction_count_by_hash(&self, hash: Hash) -> ApiResult<Option<U256>> {
        let block = self.get_block_by_hash(hash, false).await?;
        Ok(block.and_then(|b| {
            b.get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| U256::from(arr.len() as u64))
        }))
    }

    /// eth_getBlockTransactionCountByNumber
    #[instrument(skip(self))]
    pub async fn get_block_transaction_count_by_number(
        &self,
        block_id: BlockId,
    ) -> ApiResult<Option<U256>> {
        let block = self.get_block_by_number(block_id, false).await?;
        Ok(block.and_then(|b| {
            b.get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| U256::from(arr.len() as u64))
        }))
    }

    /// eth_getUncleCountByBlockHash - Always returns 0 (no uncles in PoS)
    #[instrument(skip(self))]
    pub async fn get_uncle_count_by_block_hash(&self, _hash: Hash) -> ApiResult<U256> {
        Ok(U256::ZERO)
    }

    /// eth_getUncleCountByBlockNumber - Always returns 0 (no uncles in PoS)
    #[instrument(skip(self))]
    pub async fn get_uncle_count_by_block_number(&self, _block_id: BlockId) -> ApiResult<U256> {
        Ok(U256::ZERO)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TRANSACTION DATA
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_getTransactionByHash
    #[instrument(skip(self))]
    pub async fn get_transaction_by_hash(
        &self,
        hash: Hash,
    ) -> ApiResult<Option<serde_json::Value>> {
        let result = self
            .ipc
            .request(
                "qc-03-transaction-indexing",
                RequestPayload::GetTransactionByHash(GetTransactionByHashRequest { hash }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    /// eth_getTransactionReceipt
    #[instrument(skip(self))]
    pub async fn get_transaction_receipt(
        &self,
        hash: Hash,
    ) -> ApiResult<Option<serde_json::Value>> {
        let result = self
            .ipc
            .request(
                "qc-03-transaction-indexing",
                RequestPayload::GetTransactionReceipt(GetTransactionReceiptRequest { hash }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        if result.is_null() {
            Ok(None)
        } else {
            Ok(Some(result))
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // EXECUTION
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_call - Execute call without creating transaction
    #[instrument(skip(self))]
    pub async fn call(&self, call: CallRequest, block_id: Option<BlockId>) -> ApiResult<Bytes> {
        let result = self
            .ipc
            .request(
                "qc-11-smart-contracts",
                RequestPayload::Call(CallRequestPayload {
                    call,
                    block_id: block_id.unwrap_or_default(),
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    /// eth_estimateGas - Estimate gas for transaction
    #[instrument(skip(self))]
    pub async fn estimate_gas(
        &self,
        call: CallRequest,
        block_id: Option<BlockId>,
    ) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-11-smart-contracts",
                RequestPayload::EstimateGas(EstimateGasRequest { call, block_id }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TRANSACTION SUBMISSION
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_sendRawTransaction - Submit pre-signed transaction
    ///
    /// CRITICAL: Validates RLP structure BEFORE sending to mempool.
    #[instrument(skip(self, raw_tx))]
    pub async fn send_raw_transaction(&self, raw_tx: Bytes) -> ApiResult<Hash> {
        // STEP 1: RLP pre-validation (reject garbage at the gate)
        let validated = validate_raw_transaction(raw_tx.as_slice())?;

        debug!(
            tx_hash = %validated.hash,
            sender = %validated.sender,
            nonce = validated.nonce,
            "Validated raw transaction, submitting to mempool"
        );

        // STEP 2: Create submit request with pre-computed fields
        let submit_request = crate::ipc::validation::create_submit_request(raw_tx, &validated);

        // STEP 3: Send to mempool
        let _result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::SubmitTransaction(submit_request),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        // Return transaction hash
        Ok(validated.hash)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // LOGS & EVENTS
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_getLogs - Returns logs matching filter
    #[instrument(skip(self))]
    pub async fn get_logs(&self, filter: Filter) -> ApiResult<Vec<serde_json::Value>> {
        let result = self
            .ipc
            .request(
                "qc-03-transaction-indexing",
                RequestPayload::GetLogs(GetLogsRequest { filter }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // SYNC STATUS
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_syncing - Returns sync status
    /// Routes to Node Runtime per SPEC-16 Section 3.1
    #[instrument(skip(self))]
    pub async fn syncing(&self) -> ApiResult<SyncStatus> {
        let result = self
            .ipc
            .request(
                "node-runtime",
                RequestPayload::GetSyncStatus(GetSyncStatusRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // FEE MARKET (EIP-1559)
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_maxPriorityFeePerGas - Returns suggested max priority fee per gas
    #[instrument(skip(self))]
    pub async fn max_priority_fee_per_gas(&self) -> ApiResult<U256> {
        let result = self
            .ipc
            .request(
                "qc-06-mempool",
                RequestPayload::GetMaxPriorityFeePerGas(GetMaxPriorityFeePerGasRequest),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    /// eth_feeHistory - Returns historical gas fee data
    ///
    /// Returns base fee and priority fee percentiles for a range of blocks.
    #[instrument(skip(self))]
    pub async fn fee_history(
        &self,
        block_count: U256,
        newest_block: BlockId,
        reward_percentiles: Option<Vec<f64>>,
    ) -> ApiResult<FeeHistory> {
        // Validate block_count (max 1024 per spec)
        let count = block_count.as_u64();
        if count == 0 || count > 1024 {
            return Err(ApiError::invalid_params(
                "blockCount must be between 1 and 1024",
            ));
        }

        // Validate percentiles if provided
        if let Some(ref percentiles) = reward_percentiles {
            for p in percentiles {
                if *p < 0.0 || *p > 100.0 {
                    return Err(ApiError::invalid_params(
                        "reward percentiles must be between 0 and 100",
                    ));
                }
            }
            // Check monotonically increasing
            for window in percentiles.windows(2) {
                if window[0] > window[1] {
                    return Err(ApiError::invalid_params(
                        "reward percentiles must be monotonically increasing",
                    ));
                }
            }
        }

        let result = self
            .ipc
            .request(
                "qc-02-block-storage",
                RequestPayload::GetFeeHistory(GetFeeHistoryRequest {
                    block_count: count,
                    newest_block,
                    reward_percentiles,
                }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
    }

    // ═══════════════════════════════════════════════════════════════════════
    // RECEIPTS
    // ═══════════════════════════════════════════════════════════════════════

    /// eth_getBlockReceipts - Returns all transaction receipts for a block
    ///
    /// More efficient than calling eth_getTransactionReceipt for each tx.
    #[instrument(skip(self))]
    pub async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> ApiResult<Option<Vec<serde_json::Value>>> {
        let result = self
            .ipc
            .request(
                "qc-03-transaction-indexing",
                RequestPayload::GetBlockReceipts(GetBlockReceiptsRequest { block_id }),
                None,
            )
            .await
            .map_err(|e| ApiError::new(e.code, e.message))?;

        if result.is_null() {
            Ok(None)
        } else {
            serde_json::from_value(result).map_err(|e| ApiError::internal(e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {

    // Tests would require mock IPC handler
}
