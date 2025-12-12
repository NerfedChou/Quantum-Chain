//! # QC-12: Transaction Ordering Subsystem
//!
//! DAG-based transaction ordering using Kahn's topological sort algorithm.
//! Enables parallel execution of non-conflicting transactions.
//!
//! ## Architecture
//!
//! - **Domain**: Core entities (AnnotatedTransaction, DependencyGraph, ExecutionSchedule)
//! - **Algorithms**: Kahn's sort, dependency building, conflict detection
//! - **Ports**: Inbound (TransactionOrderingApi) and Outbound (AccessPatternAnalyzer, ConflictDetector)
//! - **Application**: Service orchestration
//! - **IPC**: Handler for inter-subsystem communication
//!
//! ## Reference
//!
//! - SPEC-12-TRANSACTION-ORDERING.md
//! - System.md Subsystem 12
//! - IPC-MATRIX.md Subsystem 12

pub mod algorithms;
pub mod application;
pub mod config;
pub mod domain;
pub mod ipc;
pub mod ports;

pub use application::service::TransactionOrderingService;
pub use config::OrderingConfig;
pub use domain::entities::*;
pub use domain::errors::OrderingError;
pub use domain::value_objects::*;
pub use ipc::{OrderTransactionsRequest, OrderTransactionsResponse, TransactionOrderingHandler};
pub use ports::inbound::TransactionOrderingApi;
pub use ports::outbound::{AccessPatternAnalyzer, ConflictDetector};
