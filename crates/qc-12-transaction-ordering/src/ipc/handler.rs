//! IPC Handler for Transaction Ordering
//!
//! Reference: IPC-MATRIX.md Subsystem 12
//!
//! ## Security Boundaries
//!
//! - MUST validate sender_id == 8 (Consensus)
//! - MUST reject requests from other subsystems
//! - MUST enforce batch size limits
//! - MUST enforce edge count limits

use crate::application::service::TransactionOrderingService;
use crate::config::OrderingConfig;
use crate::domain::entities::AnnotatedTransaction;
use crate::domain::value_objects::{AccessPattern, StorageLocation};
use crate::ipc::payloads::{OrderTransactionsRequest, OrderTransactionsResponse, OrderingMetrics};
use crate::ports::inbound::TransactionOrderingApi;
use primitive_types::{H160, H256};
use std::time::Instant;
use tracing::{error, info, warn};

/// Authorized sender ID for transaction ordering requests.
/// Only Consensus (Subsystem 8) can request ordering.
const AUTHORIZED_SENDER: u8 = 8;

/// IPC Handler for Transaction Ordering.
///
/// Validates security boundaries and delegates to domain service.
pub struct TransactionOrderingHandler {
    service: TransactionOrderingService,
    config: OrderingConfig,
}

impl TransactionOrderingHandler {
    /// Create a new handler with default config.
    pub fn new() -> Self {
        Self {
            service: TransactionOrderingService::new(),
            config: OrderingConfig::default(),
        }
    }

    /// Create a new handler with custom config.
    pub fn with_config(config: OrderingConfig) -> Self {
        Self {
            service: TransactionOrderingService::with_config(config.clone()),
            config,
        }
    }

    /// Handle an OrderTransactionsRequest.
    ///
    /// ## Security (IPC-MATRIX Subsystem 12)
    ///
    /// - Validates sender_id == 8 (Consensus) ONLY
    /// - Rejects oversized batches
    /// - Rejects complexity attacks
    pub async fn handle_order_transactions(
        &self,
        sender_id: u8,
        request: OrderTransactionsRequest,
    ) -> OrderTransactionsResponse {
        let start_time = Instant::now();

        // Security: Validate sender is Consensus (Subsystem 8)
        if sender_id != AUTHORIZED_SENDER {
            warn!(
                "[qc-12] Unauthorized sender {} attempted OrderTransactionsRequest",
                sender_id
            );
            return OrderTransactionsResponse {
                correlation_id: request.correlation_id,
                success: false,
                parallel_groups: vec![],
                metrics: OrderingMetrics::default(),
                error: Some(format!(
                    "Unauthorized sender: expected {}, got {}",
                    AUTHORIZED_SENDER, sender_id
                )),
            };
        }

        // Security: Validate batch size
        if request.transaction_hashes.len() > self.config.max_batch_size {
            warn!(
                "[qc-12] Batch size {} exceeds max {}",
                request.transaction_hashes.len(),
                self.config.max_batch_size
            );
            return OrderTransactionsResponse {
                correlation_id: request.correlation_id,
                success: false,
                parallel_groups: vec![],
                metrics: OrderingMetrics::default(),
                error: Some(format!(
                    "Batch size {} exceeds max {}",
                    request.transaction_hashes.len(),
                    self.config.max_batch_size
                )),
            };
        }

        info!(
            "[qc-12] Processing OrderTransactionsRequest with {} transactions",
            request.transaction_hashes.len()
        );

        // Convert IPC payload to domain objects
        let transactions = self.convert_to_annotated_transactions(&request);

        // Delegate to domain service
        match self.service.order_transactions(transactions).await {
            Ok(schedule) => {
                let elapsed = start_time.elapsed().as_millis() as u64;

                // Convert schedule to IPC response
                let parallel_groups: Vec<Vec<[u8; 32]>> = schedule
                    .parallel_groups
                    .iter()
                    .map(|group| group.transactions.iter().map(|h| h.0).collect())
                    .collect();

                let conflicts_detected = schedule.total_transactions
                    - schedule
                        .parallel_groups
                        .first()
                        .map(|g| g.len())
                        .unwrap_or(0);

                info!(
                    "[qc-12] ✓ Ordered {} transactions into {} groups (max parallelism: {})",
                    schedule.total_transactions,
                    schedule.parallel_groups.len(),
                    schedule.max_parallelism
                );

                OrderTransactionsResponse {
                    correlation_id: request.correlation_id,
                    success: true,
                    parallel_groups,
                    metrics: OrderingMetrics {
                        total_transactions: schedule.total_transactions as u32,
                        parallel_groups: schedule.parallel_groups.len() as u32,
                        max_parallelism: schedule.max_parallelism as u32,
                        conflicts_detected: conflicts_detected as u32,
                        ordering_time_ms: elapsed,
                    },
                    error: None,
                }
            }
            Err(e) => {
                error!("[qc-12] ❌ Ordering failed: {}", e);
                OrderTransactionsResponse {
                    correlation_id: request.correlation_id,
                    success: false,
                    parallel_groups: vec![],
                    metrics: OrderingMetrics::default(),
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Convert IPC payload to domain AnnotatedTransaction objects.
    fn convert_to_annotated_transactions(
        &self,
        request: &OrderTransactionsRequest,
    ) -> Vec<AnnotatedTransaction> {
        let mut transactions = Vec::with_capacity(request.transaction_hashes.len());

        for i in 0..request.transaction_hashes.len() {
            let hash = H256::from(request.transaction_hashes[i]);
            let sender = if i < request.senders.len() {
                H160::from(request.senders[i])
            } else {
                H160::zero()
            };
            let nonce = request.nonces.get(i).copied().unwrap_or(0);

            // Build access pattern
            let mut access_pattern = AccessPattern::default();

            if i < request.read_sets.len() {
                for (addr, key) in &request.read_sets[i] {
                    access_pattern
                        .reads
                        .insert(StorageLocation::new(H160::from(*addr), H256::from(*key)));
                }
            }

            if i < request.write_sets.len() {
                for (addr, key) in &request.write_sets[i] {
                    access_pattern
                        .writes
                        .insert(StorageLocation::new(H160::from(*addr), H256::from(*key)));
                }
            }

            transactions.push(AnnotatedTransaction::new(
                hash,
                sender,
                nonce,
                access_pattern,
            ));
        }

        transactions
    }
}

impl Default for TransactionOrderingHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(tx_count: usize) -> OrderTransactionsRequest {
        OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "test".to_string(),
            transaction_hashes: (0..tx_count).map(|i| [i as u8; 32]).collect(),
            senders: (0..tx_count).map(|i| [i as u8; 20]).collect(),
            nonces: (0..tx_count).map(|i| i as u64).collect(),
            read_sets: vec![vec![]; tx_count],
            write_sets: vec![vec![]; tx_count],
        }
    }

    #[tokio::test]
    async fn test_reject_unauthorized_sender() {
        let handler = TransactionOrderingHandler::new();
        let request = make_request(5);

        // Sender 5 is not Consensus (8)
        let response = handler.handle_order_transactions(5, request).await;

        assert!(!response.success);
        assert!(response.error.unwrap().contains("Unauthorized"));
    }

    #[tokio::test]
    async fn test_accept_authorized_sender() {
        let handler = TransactionOrderingHandler::new();
        let request = make_request(5);

        // Sender 8 is Consensus
        let response = handler.handle_order_transactions(8, request).await;

        assert!(response.success);
        assert_eq!(response.metrics.total_transactions, 5);
    }

    #[tokio::test]
    async fn test_reject_oversized_batch() {
        let config = OrderingConfig {
            max_batch_size: 2,
            ..Default::default()
        };
        let handler = TransactionOrderingHandler::with_config(config);
        let request = make_request(5);

        let response = handler.handle_order_transactions(8, request).await;

        assert!(!response.success);
        assert!(response.error.unwrap().contains("Batch size"));
    }

    #[tokio::test]
    async fn test_independent_transactions_parallelize() {
        let handler = TransactionOrderingHandler::new();

        // 3 transactions, each writing to different locations
        let request = OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "test".to_string(),
            transaction_hashes: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
            senders: vec![[10u8; 20], [20u8; 20], [30u8; 20]], // Different senders
            nonces: vec![0, 0, 0],
            read_sets: vec![vec![], vec![], vec![]],
            write_sets: vec![
                vec![([1u8; 20], [1u8; 32])],
                vec![([2u8; 20], [2u8; 32])],
                vec![([3u8; 20], [3u8; 32])],
            ],
        };

        let response = handler.handle_order_transactions(8, request).await;

        assert!(response.success);
        // All independent, should be in one group
        assert_eq!(response.parallel_groups.len(), 1);
        assert_eq!(response.metrics.max_parallelism, 3);
    }

    #[tokio::test]
    async fn test_dependent_transactions_sequentialize() {
        let config = OrderingConfig {
            conflict_threshold_percent: 100, // Never fallback
            ..Default::default()
        };
        let handler = TransactionOrderingHandler::with_config(config);

        // 2 transactions, tx2 reads what tx1 writes
        let request = OrderTransactionsRequest {
            correlation_id: [1u8; 16],
            reply_to: "test".to_string(),
            transaction_hashes: vec![[1u8; 32], [2u8; 32]],
            senders: vec![[10u8; 20], [20u8; 20]], // Different senders
            nonces: vec![0, 0],
            read_sets: vec![
                vec![],
                vec![([1u8; 20], [1u8; 32])], // tx2 reads location
            ],
            write_sets: vec![
                vec![([1u8; 20], [1u8; 32])], // tx1 writes location
                vec![],
            ],
        };

        let response = handler.handle_order_transactions(8, request).await;

        assert!(response.success);
        // Dependent, should be in separate groups
        assert_eq!(response.parallel_groups.len(), 2);
    }
}
