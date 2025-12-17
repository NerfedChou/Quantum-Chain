//! # API Gateway Handler
//!
//! API Gateway integration for admin panel and JSON-RPC.
//!
//! ## Modules
//!
//! - `handler`: ApiGatewayHandler struct and methods
//! - `types`: Qc02Metrics, RpcPendingAssembly structs
//! - `security`: Request validation and sanitization

mod handler;
mod security;
#[cfg(test)]
mod tests;
mod types;

pub use handler::{handle_api_query, ApiGatewayHandler, ApiQueryError};
pub use types::{Qc02Metrics, RpcPendingAssembly};
