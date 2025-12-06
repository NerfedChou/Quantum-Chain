//! # Adapters Module
//!
//! Contains adapter implementations for the Block Storage subsystem.
//!
//! ## Modules
//!
//! - `api_handler`: API Gateway integration for admin panel and JSON-RPC

pub mod api_handler;

pub use api_handler::{
    handle_api_query, ApiGatewayHandler, ApiQueryError, Qc02Metrics, RpcPendingAssembly,
};
