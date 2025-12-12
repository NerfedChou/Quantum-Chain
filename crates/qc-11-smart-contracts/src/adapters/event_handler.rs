//! # Event Handler Adapter
//!
//! Handles inbound IPC events and publishes results to Event Bus.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - Subscribes to execution request topics
//! - Validates sender IDs per IPC-MATRIX.md
//! - Publishes execution results
//! - Uses correlation IDs for request/response matching

use crate::domain::value_objects::Bytes;
use crate::errors::IpcError;
use crate::events::{
    subsystem_ids, ExecuteHTLCRequestPayload, ExecuteHTLCResponsePayload,
    ExecuteTransactionRequestPayload, ExecuteTransactionResponsePayload,
};
use crate::ports::inbound::{SignedTransaction, SmartContractApi};
use std::sync::Arc;

/// Event handler for Smart Contract execution requests.
///
/// ## IPC-MATRIX.md Compliance
///
/// - Validates `sender_id` before processing
/// - Uses envelope `correlation_id` for responses
/// - NO `requester_id` fields in payloads
pub struct SmartContractEventHandler<T: SmartContractApi> {
    /// The smart contract API implementation.
    api: Arc<T>,
}

impl<T: SmartContractApi> SmartContractEventHandler<T> {
    /// Create a new event handler.
    pub fn new(api: Arc<T>) -> Self {
        Self { api }
    }

    /// Handle an `ExecuteTransactionRequest`.
    ///
    /// ## Security
    ///
    /// - Validates `sender_id` is 8 (Consensus) or 12 (Transaction Ordering)
    /// - Rejects unauthorized senders
    pub async fn handle_execute_transaction(
        &self,
        sender_id: u8,
        payload: ExecuteTransactionRequestPayload,
    ) -> Result<ExecuteTransactionResponsePayload, IpcError> {
        // Validate sender (IPC-MATRIX.md)
        if !subsystem_ids::is_authorized_execution_sender(sender_id) {
            return Err(IpcError::UnauthorizedSender {
                sender_id,
                allowed: vec![
                    subsystem_ids::CONSENSUS,
                    subsystem_ids::TRANSACTION_ORDERING,
                ],
            });
        }

        // Convert payload to SignedTransaction
        let tx = SignedTransaction {
            from: payload.from,
            to: payload.to,
            value: payload.value,
            nonce: payload.nonce,
            gas_price: payload.gas_price,
            gas_limit: payload.gas_limit,
            data: payload.data,
            hash: payload.tx_hash,
        };

        // Execute
        let result = self
            .api
            .execute_transaction(&tx, &payload.block_context)
            .await;

        // Convert result to response payload
        match result {
            Ok(result) => Ok(ExecuteTransactionResponsePayload {
                success: result.success,
                gas_used: result.gas_used,
                output: result.output,
                logs: result.logs,
                state_changes: result.state_changes,
                contract_address: None, // TODO: extract from state changes
                revert_reason: result.revert_reason,
            }),
            Err(err) => {
                // Execution failed, still return a response
                Ok(ExecuteTransactionResponsePayload {
                    success: false,
                    gas_used: 0, // TODO: track gas on failure
                    output: Bytes::new(),
                    logs: vec![],
                    state_changes: vec![],
                    contract_address: None,
                    revert_reason: Some(err.to_string()),
                })
            }
        }
    }

    /// Handle an `ExecuteHTLCRequest`.
    ///
    /// ## Security
    ///
    /// - Validates `sender_id` is 15 (Cross-Chain) ONLY
    pub async fn handle_execute_htlc(
        &self,
        sender_id: u8,
        _payload: ExecuteHTLCRequestPayload,
    ) -> Result<ExecuteHTLCResponsePayload, IpcError> {
        // Validate sender (IPC-MATRIX.md)
        if !subsystem_ids::is_authorized_htlc_sender(sender_id) {
            return Err(IpcError::UnauthorizedSender {
                sender_id,
                allowed: vec![subsystem_ids::CROSS_CHAIN],
            });
        }

        // HTLC execution would be implemented here
        // For now, return a placeholder response
        Ok(ExecuteHTLCResponsePayload {
            success: false,
            gas_used: 0,
            revert_reason: Some("HTLC execution not yet implemented".to_string()),
        })
    }
}

/// Trait for event bus integration.
///
/// This trait would be implemented to connect to the actual shared-bus crate.
#[async_trait::async_trait]
pub trait EventBusAdapter: Send + Sync {
    /// Subscribe to a topic.
    async fn subscribe(&self, topic: &str) -> Result<(), IpcError>;

    /// Publish a message to a topic.
    async fn publish<T: serde::Serialize + Send>(
        &self,
        topic: &str,
        correlation_id: uuid::Uuid,
        payload: T,
    ) -> Result<(), IpcError>;

    /// Get the next message from subscriptions.
    async fn receive(&self) -> Option<InboundMessage>;
}

/// Inbound message from event bus.
pub struct InboundMessage {
    /// Topic the message was received on.
    pub topic: String,
    /// Sender subsystem ID (from envelope).
    pub sender_id: u8,
    /// Correlation ID for request/response matching.
    pub correlation_id: uuid::Uuid,
    /// Raw payload bytes.
    pub payload: Vec<u8>,
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{BlockContext, ExecutionContext, ExecutionResult};
    use crate::domain::value_objects::{Address, Bytes, U256};
    use crate::errors::VmError;
    use crate::events::HtlcOperationPayload;
    use crate::ports::inbound::SignedTransaction;

    // Mock SmartContractApi for testing
    struct MockApi;

    #[async_trait::async_trait]
    impl SmartContractApi for MockApi {
        async fn execute(
            &self,
            _context: ExecutionContext,
            _code: &[u8],
        ) -> Result<ExecutionResult, VmError> {
            Ok(ExecutionResult::success(Bytes::new(), 21000))
        }

        async fn execute_transaction(
            &self,
            _tx: &SignedTransaction,
            _block: &BlockContext,
        ) -> Result<ExecutionResult, VmError> {
            Ok(ExecutionResult::success(Bytes::new(), 21000))
        }

        async fn estimate_gas(
            &self,
            _context: ExecutionContext,
            _code: &[u8],
        ) -> Result<u64, VmError> {
            Ok(21000)
        }

        async fn call(&self, _context: ExecutionContext, _code: &[u8]) -> Result<Bytes, VmError> {
            Ok(Bytes::new())
        }
    }

    #[tokio::test]
    async fn test_authorized_execution_request() {
        let handler = SmartContractEventHandler::new(Arc::new(MockApi));

        let payload = ExecuteTransactionRequestPayload {
            from: Address::new([1u8; 20]),
            to: Some(Address::new([2u8; 20])),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 21000,
            data: Bytes::new(),
            tx_hash: crate::domain::value_objects::Hash::ZERO,
            block_context: BlockContext::default(),
        };

        // Consensus (8) is authorized
        let result = handler.handle_execute_transaction(8, payload.clone()).await;
        assert!(result.is_ok());

        // Transaction Ordering (12) is authorized
        let result = handler
            .handle_execute_transaction(12, payload.clone())
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unauthorized_execution_request() {
        let handler = SmartContractEventHandler::new(Arc::new(MockApi));

        let payload = ExecuteTransactionRequestPayload {
            from: Address::new([1u8; 20]),
            to: Some(Address::new([2u8; 20])),
            value: U256::zero(),
            nonce: 0,
            gas_price: U256::from(1),
            gas_limit: 21000,
            data: Bytes::new(),
            tx_hash: crate::domain::value_objects::Hash::ZERO,
            block_context: BlockContext::default(),
        };

        // Cross-Chain (15) is NOT authorized for execution
        let result = handler.handle_execute_transaction(15, payload).await;
        assert!(matches!(result, Err(IpcError::UnauthorizedSender { .. })));
    }

    #[tokio::test]
    async fn test_authorized_htlc_request() {
        let handler = SmartContractEventHandler::new(Arc::new(MockApi));

        let payload = ExecuteHTLCRequestPayload {
            htlc_contract: Address::new([1u8; 20]),
            operation: HtlcOperationPayload::Refund,
            block_context: BlockContext::default(),
        };

        // Cross-Chain (15) is authorized
        let result = handler.handle_execute_htlc(15, payload).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unauthorized_htlc_request() {
        let handler = SmartContractEventHandler::new(Arc::new(MockApi));

        let payload = ExecuteHTLCRequestPayload {
            htlc_contract: Address::new([1u8; 20]),
            operation: HtlcOperationPayload::Refund,
            block_context: BlockContext::default(),
        };

        // Consensus (8) is NOT authorized for HTLC
        let result = handler.handle_execute_htlc(8, payload).await;
        assert!(matches!(result, Err(IpcError::UnauthorizedSender { .. })));
    }
}
