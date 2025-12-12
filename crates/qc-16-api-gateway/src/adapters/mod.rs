//! Adapters for the API Gateway.
//!
//! Infrastructure implementations for async operations and external integrations.

pub mod pending;

pub use pending::{cleanup_task, PendingRequestStore, SubsystemResponse};
