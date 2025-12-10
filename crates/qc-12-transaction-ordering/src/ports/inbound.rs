//! Inbound Ports (Driving Ports / API)
//!
//! Reference: SPEC-12 Section 3.1 (Lines 201-228)

use crate::domain::entities::{AnnotatedTransaction, DependencyGraph, ExecutionSchedule};
use crate::domain::errors::OrderingError;
use async_trait::async_trait;

/// Primary Transaction Ordering API
///
/// Reference: SPEC-12 Lines 203-228
#[async_trait]
pub trait TransactionOrderingApi: Send + Sync {
    /// Analyze and order transactions for parallel execution.
    ///
    /// This is the main entry point. It:
    /// 1. Annotates transactions with access patterns
    /// 2. Builds the dependency graph
    /// 3. Performs topological sort
    /// 4. Returns the execution schedule
    async fn order_transactions(
        &self,
        transactions: Vec<AnnotatedTransaction>,
    ) -> Result<ExecutionSchedule, OrderingError>;

    /// Build dependency graph for transactions.
    ///
    /// Pure function that analyzes conflicts and nonce ordering.
    fn build_dependency_graph(
        &self,
        transactions: Vec<AnnotatedTransaction>,
    ) -> Result<DependencyGraph, OrderingError>;

    /// Get parallel execution schedule from graph.
    ///
    /// Performs Kahn's topological sort.
    fn schedule_parallel_execution(
        &self,
        graph: &DependencyGraph,
    ) -> Result<ExecutionSchedule, OrderingError>;
}
