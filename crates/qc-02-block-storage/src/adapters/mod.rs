//! # Adapters Module
//!
//! Contains adapter implementations for the Block Storage subsystem.
//!
//! ## Modules
//!
//! - `api_handler`: API Gateway integration for admin panel and JSON-RPC
//! - `lock`: Database process locking (singleton guard)

pub mod api_handler;
pub mod lock;

pub use api_handler::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly,
};
pub use lock::{DatabaseLock, LockError};
