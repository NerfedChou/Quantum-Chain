//! Transaction Ordering Service
//!
//! Main service implementing TransactionOrderingApi.
//! Reference: Architecture.md Section 2.1, SPEC-12

use crate::algorithms::{build_dependency_graph, conflict_detector, kahns_topological_sort};
use crate::config::OrderingConfig;
use crate::domain::entities::{AnnotatedTransaction, DependencyGraph, ExecutionSchedule};
use crate::domain::errors::OrderingError;
use crate::ports::inbound::TransactionOrderingApi;
use async_trait::async_trait;

use tracing::{debug, info, warn};

/// Transaction Ordering Service
///
/// Orchestrates the ordering pipeline:
/// 1. Validate input
/// 2. Build dependency graph
/// 3. Check conflict threshold
/// 4. Execute Kahn's algorithm
/// 5. Return execution schedule
pub struct TransactionOrderingService {
    config: OrderingConfig,
}

impl TransactionOrderingService {
    /// Create a new service with default config
    pub fn new() -> Self {
        Self {
            config: OrderingConfig::default(),
        }
    }

    /// Create a new service with custom config
    pub fn with_config(config: OrderingConfig) -> Self {
        Self { config }
    }

    /// Validate batch size and edge count
    fn validate_batch(&self, transactions: &[AnnotatedTransaction]) -> Result<(), OrderingError> {
        if transactions.is_empty() {
            return Err(OrderingError::EmptyBatch);
        }

        if transactions.len() > self.config.max_batch_size {
            return Err(OrderingError::BatchTooLarge {
                size: transactions.len(),
                max: self.config.max_batch_size,
            });
        }

        Ok(())
    }

    /// Check if we should fallback to sequential execution
    fn should_fallback(
        &self,
        conflicts: &[crate::domain::value_objects::Conflict],
        tx_count: usize,
    ) -> bool {
        let percent = conflict_detector::conflict_percentage(conflicts, tx_count);
        percent > self.config.conflict_threshold_percent
    }
}

impl Default for TransactionOrderingService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TransactionOrderingApi for TransactionOrderingService {
    async fn order_transactions(
        &self,
        transactions: Vec<AnnotatedTransaction>,
    ) -> Result<ExecutionSchedule, OrderingError> {
        // 1. Validate input
        self.validate_batch(&transactions)?;

        info!(
            tx_count = transactions.len(),
            "Ordering transactions for parallel execution"
        );

        // 2. Detect conflicts
        let conflicts = conflict_detector::detect_conflicts(&transactions);
        debug!(conflict_count = conflicts.len(), "Detected conflicts");

        // 3. Check conflict threshold for fallback
        if self.should_fallback(&conflicts, transactions.len()) {
            warn!(
                conflict_percent =
                    conflict_detector::conflict_percentage(&conflicts, transactions.len()),
                threshold = self.config.conflict_threshold_percent,
                "Conflict threshold exceeded, falling back to sequential"
            );

            let hashes: Vec<_> = transactions.iter().map(|tx| tx.hash).collect();
            return Ok(ExecutionSchedule::sequential(hashes));
        }

        // 4. Build dependency graph
        let graph = self.build_dependency_graph(transactions)?;

        // 5. Validate edge count
        if graph.edge_count() > self.config.max_edge_count {
            return Err(OrderingError::TooManyEdges {
                count: graph.edge_count(),
                max: self.config.max_edge_count,
            });
        }

        // 6. Schedule parallel execution
        let schedule = self.schedule_parallel_execution(&graph)?;

        info!(
            total_transactions = schedule.total_transactions,
            parallel_groups = schedule.parallel_groups.len(),
            max_parallelism = schedule.max_parallelism,
            "Transaction ordering complete"
        );

        Ok(schedule)
    }

    fn build_dependency_graph(
        &self,
        transactions: Vec<AnnotatedTransaction>,
    ) -> Result<DependencyGraph, OrderingError> {
        let graph = build_dependency_graph(transactions);
        Ok(graph)
    }

    fn schedule_parallel_execution(
        &self,
        graph: &DependencyGraph,
    ) -> Result<ExecutionSchedule, OrderingError> {
        kahns_topological_sort(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::{AccessPattern, StorageLocation};
    use primitive_types::{H160, H256};

    fn make_hash(val: u8) -> H256 {
        H256::from_low_u64_be(val as u64)
    }

    fn make_addr(val: u8) -> H160 {
        H160::from_low_u64_be(val as u64)
    }

    fn loc(addr: u8, key: u8) -> StorageLocation {
        StorageLocation::new(make_addr(addr), make_hash(key))
    }

    #[tokio::test]
    async fn test_order_independent_transactions() {
        let service = TransactionOrderingService::new();

        let tx1 = AnnotatedTransaction::new(
            make_hash(1),
            make_addr(10),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );
        let tx2 = AnnotatedTransaction::new(
            make_hash(2),
            make_addr(20),
            0,
            AccessPattern::new().with_writes(vec![loc(2, 2)]),
        );

        let schedule = service.order_transactions(vec![tx1, tx2]).await.unwrap();

        // Both independent, should be in one parallel group
        assert_eq!(schedule.parallel_groups.len(), 1);
        assert_eq!(schedule.max_parallelism, 2);
    }

    #[tokio::test]
    async fn test_order_dependent_transactions() {
        let service = TransactionOrderingService::new();

        let tx1 = AnnotatedTransaction::new(
            make_hash(1),
            make_addr(10),
            0,
            AccessPattern::new().with_writes(vec![loc(1, 1)]),
        );
        let tx2 = AnnotatedTransaction::new(
            make_hash(2),
            make_addr(20),
            0,
            AccessPattern::new().with_reads(vec![loc(1, 1)]),
        );

        let schedule = service
            .order_transactions(vec![tx1.clone(), tx2.clone()])
            .await
            .unwrap();

        // tx2 depends on tx1, should be in separate groups
        assert_eq!(schedule.parallel_groups.len(), 2);
        assert_eq!(schedule.flatten()[0], tx1.hash);
        assert_eq!(schedule.flatten()[1], tx2.hash);
    }

    #[tokio::test]
    async fn test_reject_empty_batch() {
        let service = TransactionOrderingService::new();

        let result = service.order_transactions(vec![]).await;

        assert!(matches!(result, Err(OrderingError::EmptyBatch)));
    }

    #[tokio::test]
    async fn test_reject_oversized_batch() {
        let config = OrderingConfig {
            max_batch_size: 2,
            ..Default::default()
        };
        let service = TransactionOrderingService::with_config(config);

        let transactions: Vec<_> = (0..5)
            .map(|i| AnnotatedTransaction::new(make_hash(i), make_addr(i), 0, AccessPattern::new()))
            .collect();

        let result = service.order_transactions(transactions).await;

        assert!(matches!(result, Err(OrderingError::BatchTooLarge { .. })));
    }

    #[tokio::test]
    async fn test_fallback_to_sequential_on_high_conflicts() {
        let config = OrderingConfig {
            conflict_threshold_percent: 10, // Very low threshold
            ..Default::default()
        };
        let service = TransactionOrderingService::with_config(config);

        // All transactions write to same slot = 100% conflicts
        let transactions: Vec<_> = (0..5)
            .map(|i| {
                AnnotatedTransaction::new(
                    make_hash(i),
                    make_addr(i),
                    0,
                    AccessPattern::new().with_writes(vec![loc(1, 1)]),
                )
            })
            .collect();

        let schedule = service.order_transactions(transactions).await.unwrap();

        // Should fallback to sequential (1 tx per group)
        assert_eq!(schedule.max_parallelism, 1);
        assert_eq!(schedule.parallel_groups.len(), 5);
    }

    #[tokio::test]
    async fn test_nonce_ordering_preserved() {
        // Use high conflict threshold to ensure we go through the graph algorithm
        let config = OrderingConfig {
            conflict_threshold_percent: 100,
            ..Default::default()
        };
        let service = TransactionOrderingService::with_config(config);
        let sender = make_addr(10);

        let tx1 = AnnotatedTransaction::new(make_hash(1), sender, 0, AccessPattern::new());
        let tx2 = AnnotatedTransaction::new(make_hash(2), sender, 1, AccessPattern::new());
        let tx3 = AnnotatedTransaction::new(make_hash(3), sender, 2, AccessPattern::new());

        let schedule = service
            .order_transactions(vec![tx3.clone(), tx1.clone(), tx2.clone()]) // Out of order input
            .await
            .unwrap();

        // Should be ordered by nonce
        let flat = schedule.flatten();
        let idx1 = flat.iter().position(|h| *h == tx1.hash).unwrap();
        let idx2 = flat.iter().position(|h| *h == tx2.hash).unwrap();
        let idx3 = flat.iter().position(|h| *h == tx3.hash).unwrap();

        assert!(idx1 < idx2);
        assert!(idx2 < idx3);
    }
}
