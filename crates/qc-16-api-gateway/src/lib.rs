// Allow missing docs for internal items in development
#![allow(missing_docs)]

//! QC-16 API Gateway - External interface for JSON-RPC, WebSocket, and REST APIs.
//!
//! This crate provides the public API for the Quantum Chain blockchain.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                           API GATEWAY (qc-16)                                │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                         │
//! │  │   HTTP/RPC  │  │  WebSocket  │  │    Admin    │                         │
//! │  │  Port 8545  │  │  Port 8546  │  │  Port 8080  │                         │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                         │
//! │         │                │                │                                 │
//! │  ┌──────┴────────────────┴────────────────┴──────┐                         │
//! │  │              Middleware Stack                  │                         │
//! │  │  RateLimit → Validation → Auth → Timeout      │                         │
//! │  └────────────────────┬───────────────────────────┘                         │
//! │                       │                                                     │
//! │  ┌────────────────────┴───────────────────────┐                            │
//! │  │           Pending Request Store            │                            │
//! │  │     (Async-to-Sync Bridge via oneshot)     │                            │
//! │  └────────────────────┬───────────────────────┘                            │
//! │                       │                                                     │
//! │  ┌────────────────────┴───────────────────────┐                            │
//! │  │              IPC Handler                    │                            │
//! │  │        (Event Bus Integration)             │                            │
//! │  └────────────────────┬───────────────────────┘                            │
//! └───────────────────────┼─────────────────────────────────────────────────────┘
//!                         │
//!                    Event Bus
//!                         │
//!     ┌───────────────────┼───────────────────────┐
//!     ▼                   ▼                       ▼
//! qc-04-state      qc-02-block           qc-06-mempool
//! ```
//!
//! # Method Tiers
//!
//! - **Tier 1 (Public)**: No authentication required (eth_*, web3_*, net_*)
//! - **Tier 2 (Protected)**: API key OR localhost (txpool_*, admin_* read-only)
//! - **Tier 3 (Admin)**: Localhost AND API key (admin_* write, debug_*)
//!
//! # Usage
//!
//! ```ignore
//! use qc_16_api_gateway::{ApiGatewayService, GatewayConfig};
//!
//! let config = GatewayConfig::default();
//! let mut service = ApiGatewayService::new(config, ipc_sender, data_dir)?;
//! service.start().await?;
//! ```
//!
//! # Security
//!
//! - RLP pre-validation for eth_sendRawTransaction (reject garbage at the gate)
//! - Per-IP rate limiting with token bucket algorithm
//! - Method tier enforcement with API key / localhost checks
//! - Request size and batch limits
//! - Per-method timeouts
//!
//! # SPEC Reference
//!
//! See [SPEC-16-API-GATEWAY.md](../../SPECS/SPEC-16-API-GATEWAY.md) for full specification.

// Note: missing_docs is disabled temporarily for Docker builds with -D warnings
// #![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_code)]

pub mod domain;
pub mod ipc;
pub mod middleware;
pub mod ports;
pub mod rpc;
pub mod service;
pub mod ws;

// Re-exports for public API
pub use domain::config::GatewayConfig;
pub use domain::error::{ApiError, ApiResult, GatewayError};
pub use domain::methods::{get_method_info, is_method_supported, MethodInfo, MethodTier};
pub use domain::types::*;
pub use ipc::{IpcHandler, IpcRequest, IpcResponse, IpcSender};
pub use middleware::GatewayMetrics;
pub use service::ApiGatewayService;
pub use ws::SubscriptionManager;

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Client version string for web3_clientVersion
pub fn client_version() -> String {
    format!("QuantumChain/v{}/linux/rust", VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_client_version() {
        let version = client_version();
        assert!(version.starts_with("QuantumChain/"));
        assert!(version.contains(VERSION));
    }

    #[test]
    fn test_method_support() {
        assert!(is_method_supported("eth_getBalance"));
        assert!(is_method_supported("eth_sendRawTransaction"));
        assert!(is_method_supported("web3_clientVersion"));
        assert!(!is_method_supported("eth_fakeMethod"));
    }
}
