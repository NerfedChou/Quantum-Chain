//! # Transaction Ordering Adapter
//!
//! Adapter that wraps qc-12-transaction-ordering domain logic and connects
//! it to the choreography event bus.
//!
//! ## IPC-MATRIX Subsystem 12
//!
//! - Receives: OrderTransactionsRequest from Consensus (8)
//! - Queries: State Management (4) for conflict detection
//! - Sends: OrderedTransactions to Smart Contracts (11)

use std::sync::Arc;
use tracing::{debug, error, info};

use qc_12_transaction_ordering::{
    OrderTransactionsRequest, OrderTransactionsResponse, OrderingConfig, TransactionOrderingHandler,
};
use shared_types::SubsystemId;

use crate::adapters::EventBusAdapter;
use crate::wiring::EventRouter;

/// Transaction Ordering adapter - wraps qc-12 domain logic.
///
/// Handles transaction ordering requests from Consensus and coordinates
/// with State Management for conflict detection.
pub struct TransactionOrderingAdapter {
    /// IPC Handler from qc-12 (contains domain service)
    handler: TransactionOrderingHandler,
    /// Event bus for publishing results
    event_bus: EventBusAdapter,
}

impl TransactionOrderingAdapter {
    /// Create a new adapter with default configuration.
    pub fn new(router: Arc<EventRouter>) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::TransactionOrdering);
        let handler = TransactionOrderingHandler::new();

        Self { handler, event_bus }
    }

    /// Create with custom configuration.
    pub fn with_config(router: Arc<EventRouter>, config: OrderingConfig) -> Self {
        let event_bus = EventBusAdapter::new(router, SubsystemId::TransactionOrdering);
        let handler = TransactionOrderingHandler::with_config(config);

        Self { handler, event_bus }
    }

    /// Process an OrderTransactionsRequest from Consensus.
    ///
    /// ## IPC-MATRIX Security
    ///
    /// - Validates sender_id == 8 (Consensus)
    /// - Enforces batch size limits
    /// - Handles cycle detection
    ///
    /// ## Event Publishing
    ///
    /// On success, publishes `TransactionsOrdered` event for Smart Contracts (11).
    pub async fn process_order_transactions(
        &self,
        sender_id: SubsystemId,
        request: OrderTransactionsRequest,
        block_hash: [u8; 32],
        block_height: u64,
    ) -> OrderTransactionsResponse {
        info!(
            "[qc-12] Processing order request with {} transactions from {:?}",
            request.transaction_hashes.len(),
            sender_id
        );

        // Query State Management for conflicts (currently returns empty - WIP)
        if let Err(e) = self.query_state_for_conflicts(&request.transaction_hashes).await {
            error!("[qc-12] Failed to query state for conflicts: {}", e);
        }

        // Delegate to IPC handler (which validates sender and calls domain)
        let response = self
            .handler
            .handle_order_transactions(sender_id.as_u8(), request)
            .await;

        if response.success {
            info!(
                "[qc-12] ✓ Ordered {} txs into {} parallel groups (max parallelism: {})",
                response.metrics.total_transactions,
                response.metrics.parallel_groups,
                response.metrics.max_parallelism
            );

            // Publish TransactionsOrdered event for Smart Contracts (11)
            use crate::wiring::ChoreographyEvent;
            let event = ChoreographyEvent::TransactionsOrdered {
                block_hash,
                block_height,
                parallel_groups: response.parallel_groups.clone(),
                max_parallelism: response.metrics.max_parallelism,
                sender_id: SubsystemId::TransactionOrdering,
            };

            if let Err(e) = self.event_bus.publish(event) {
                error!("[qc-12] Failed to publish TransactionsOrdered event: {}", e);
            } else {
                info!(
                    "[qc-12] Published TransactionsOrdered for block {} to Smart Contracts",
                    block_height
                );
            }
        } else {
            error!("[qc-12] ❌ Ordering failed: {:?}", response.error);
        }

        response
    }

    /// Query State Management (4) for conflict detection.
    ///
    /// This is an outbound call to Subsystem 4.
    ///
    /// # TODO
    ///
    /// Implement full conflict detection flow:
    /// 1. Send ConflictDetectionRequest to State Management (4) via event bus
    /// 2. Wait for ConflictDetectionResponse with timeout
    /// 3. Return the detected conflicts (tx_index_a, tx_index_b, conflict_type)
    ///
    /// Currently returns empty conflicts (no conflict detection).
    #[allow(clippy::unused_async)] // Will be async when implemented
    async fn query_state_for_conflicts(
        &self,
        _tx_hashes: &[[u8; 32]],
    ) -> Result<Vec<(usize, usize, u8)>, TransactionOrderingAdapterError> {
        debug!("[qc-12] Querying State Management for conflicts (stub - returns empty)");
        Ok(vec![])
    }

    /// Get the handler for direct access (testing).
    #[cfg(test)]
    pub fn handler(&self) -> &TransactionOrderingHandler {
        &self.handler
    }
}

/// Transaction ordering adapter errors.
#[derive(Debug)]
pub enum TransactionOrderingAdapterError {
    /// Failed to publish event.
    PublishFailed(String),
    /// State query failed.
    StateQueryFailed(String),
    /// Ordering error from domain.
    OrderingError(String),
}

impl std::fmt::Display for TransactionOrderingAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PublishFailed(msg) => write!(f, "Publish failed: {}", msg),
            Self::StateQueryFailed(msg) => write!(f, "State query failed: {}", msg),
            Self::OrderingError(msg) => write!(f, "Ordering error: {}", msg),
        }
    }
}

impl std::error::Error for TransactionOrderingAdapterError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wiring::EventRouter;

    fn create_test_adapter() -> TransactionOrderingAdapter {
        let router = Arc::new(EventRouter::new(16));
        TransactionOrderingAdapter::new(router)
    }

    #[tokio::test]
    async fn test_adapter_rejects_unauthorized_sender() {
        let adapter = create_test_adapter();

        let request = OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "test".to_string(),
            transaction_hashes: vec![[1u8; 32]],
            senders: vec![[1u8; 20]],
            nonces: vec![0],
            read_sets: vec![vec![]],
            write_sets: vec![vec![]],
        };

        // Mempool (6) is not authorized
        let response = adapter
            .process_order_transactions(
                SubsystemId::Mempool,
                request,
                [0u8; 32], // block_hash
                1,         // block_height
            )
            .await;

        assert!(!response.success);
        assert!(response.error.unwrap().contains("Unauthorized"));
    }

    #[tokio::test]
    async fn test_adapter_accepts_consensus() {
        let adapter = create_test_adapter();

        let request = OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "test".to_string(),
            transaction_hashes: vec![[1u8; 32], [2u8; 32]],
            senders: vec![[1u8; 20], [2u8; 20]],
            nonces: vec![0, 0],
            read_sets: vec![vec![], vec![]],
            write_sets: vec![vec![], vec![]],
        };

        // Consensus (8) is authorized
        let response = adapter
            .process_order_transactions(
                SubsystemId::Consensus,
                request,
                [0u8; 32], // block_hash
                42,        // block_height
            )
            .await;

        assert!(response.success);
        assert_eq!(response.metrics.total_transactions, 2);
    }
}
