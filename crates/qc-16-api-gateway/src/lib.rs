//! # QC-16 API Gateway - External Interface Subsystem
//!
//! **Subsystem ID:** 16  
//! **Specification:** SPEC-16-API-GATEWAY.md v1.1  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Security Level:** CRITICAL (External-Facing)  
//! **Status:** Production-Ready (Phase 3)
//!
//! ## Purpose
//!
//! The API Gateway is the **single entry point** for all external interactions
//! with the Quantum Chain node. It translates external protocols (JSON-RPC,
//! WebSocket, REST) into internal event bus messages, enforcing security
//! boundaries at the network edge.
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INV-1 | Rate Limit Enforcement | `middleware/rate_limit.rs:102-125` - `RateLimitState::check()` |
//! | INV-2 | Method Tier Authorization | `middleware/auth.rs:108-145` - `AuthService::call()` |
//! | INV-3 | Request Size Limits | `middleware/validation.rs:75-100` - `ValidationService::call()` |
//! | INV-4 | RLP Pre-validation | `ipc/validation.rs:30-120` - `validate_transaction_rlp()` |
//! | INV-5 | Timeout Enforcement | `middleware/timeout.rs:40-80` - `TimeoutService::call()` |
//!
//! ## Security (SPEC-16 Section 5)
//!
//! ### Defense in Depth Layers
//!
//! ```text
//! [Request] → Rate Limit → Size Limit → Auth → Timeout → Handler
//! ```
//!
//! | Layer | Purpose | Enforcement |
//! |-------|---------|-------------|
//! | Rate Limiting | DDoS prevention | `middleware/rate_limit.rs` - Token bucket per IP |
//! | Size Limits | Memory exhaustion prevention | `middleware/validation.rs` - Max 1MB request |
//! | Authentication | Access control | `middleware/auth.rs` - API key + localhost checks |
//! | Timeout | Resource exhaustion prevention | `middleware/timeout.rs` - Per-method limits |
//! | RLP Validation | Garbage rejection | `ipc/validation.rs` - Syntactic check before IPC |
//!
//! ### Method Tier Matrix
//!
//! | Tier | Methods | Auth Required | Localhost Required |
//! |------|---------|---------------|-------------------|
//! | Public | `eth_*`, `web3_*`, `net_version` | ❌ | ❌ |
//! | Protected | `txpool_*`, `net_peerCount` | API Key OR Localhost | ❌ |
//! | Admin | `admin_*`, `debug_*`, `miner_*` | API Key | ✅ |
//!
//! ### Constant-Time API Key Comparison
//!
//! Uses `subtle::ConstantTimeEq` to prevent timing attacks:
//! - Location: `middleware/auth.rs:228-248` - `constant_time_compare()`
//!
//! ## IPC Authorization (Outbound Only)
//!
//! API Gateway SENDS messages TO other subsystems (it does not receive IPC):
//!
//! | Target Subsystem | Message Types | Purpose |
//! |-----------------|---------------|---------|
//! | qc-01 Peer Discovery | `GetPeersRequest`, `AddPeerRequest` | Peer management |
//! | qc-02 Block Storage | `ReadBlockRequest`, `ReadBlockRangeRequest` | Block queries |
//! | qc-03 Transaction Indexing | `GetTransactionRequest`, `GetLogsRequest` | Tx/receipt queries |
//! | qc-04 State Management | `StateReadRequest`, `BalanceCheckRequest` | State queries |
//! | qc-06 Mempool | `AddTransactionRequest`, `GetMempoolStatusRequest` | Tx submission |
//! | qc-08 Consensus | `StartMiningRequest`, `StopMiningRequest` | Block production (Admin) |
//! | qc-10 Signature Verify | `VerifyTransactionRequest` | Tx signature validation |
//! | qc-11 Smart Contracts | `ExecuteCallRequest`, `EstimateGasRequest` | eth_call/estimateGas |
//!
//! **IMPORTANT:** Internal IPC messages do NOT require cryptographic signatures.
//! The Event Bus uses in-memory channels which are process-private (SPEC v1.1 Fix).
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                           API GATEWAY (qc-16)                                │
//! ├─────────────────────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
//! │  │   HTTP/RPC  │  │  WebSocket  │  │    Admin    │  │   Health    │        │
//! │  │  Port 8545  │  │  Port 8546  │  │  Port 8080  │  │  Port 8081  │        │
//! │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │
//! │         │                │                │                │                │
//! │  ┌──────┴────────────────┴────────────────┴────────────────┴──────┐        │
//! │  │                    Middleware Stack                             │        │
//! │  │  RateLimit → Validation → Auth → CORS → Timeout → Tracing     │        │
//! │  └────────────────────────────┬───────────────────────────────────┘        │
//! │                               │                                            │
//! │  ┌────────────────────────────┴───────────────────────┐                    │
//! │  │              Pending Request Store                  │                    │
//! │  │        (Async-to-Sync Bridge via oneshot)          │                    │
//! │  │  Implements: Correlation ID → Response Channel     │                    │
//! │  └────────────────────────────┬───────────────────────┘                    │
//! │                               │                                            │
//! │  ┌────────────────────────────┴───────────────────────┐                    │
//! │  │                  IPC Handler                        │                    │
//! │  │           (Event Bus Integration)                  │                    │
//! │  └────────────────────────────┬───────────────────────┘                    │
//! └───────────────────────────────┼────────────────────────────────────────────┘
//!                                 │
//!                            Event Bus
//!                                 │
//!     ┌───────────────────────────┼───────────────────────┐
//!     ▼                           ▼                       ▼
//! qc-04-state              qc-02-block            qc-06-mempool
//! ```
//!
//! ## Rate Limiting Configuration
//!
//! | Category | Default Limit | Methods |
//! |----------|--------------|---------|
//! | Public | 100 req/s | Read operations |
//! | Write | 10 req/s | `eth_sendRawTransaction` |
//! | Heavy | 20 req/s | `eth_call`, `eth_getLogs` |
//!
//! ## Timeout Configuration
//!
//! | Category | Default | Methods |
//! |----------|---------|---------|
//! | Simple | 5s | `eth_getBalance`, `eth_blockNumber` |
//! | Normal | 10s | `eth_getBlock`, `eth_getTransaction` |
//! | Heavy | 30s | `eth_call`, `eth_getLogs` |
//!
//! ## Usage Example
//!
//! ```ignore
//! use qc_16_api_gateway::{ApiGatewayService, GatewayConfig};
//!
//! // Create gateway with default configuration
//! let config = GatewayConfig::default();
//! let mut service = ApiGatewayService::new(config, ipc_sender, data_dir)?;
//!
//! // Start all servers (HTTP, WebSocket, Admin)
//! service.start().await?;
//!
//! // Graceful shutdown
//! service.shutdown();
//! ```
//!
//! ## SPEC Reference
//!
//! See [SPEC-16-API-GATEWAY.md](../../SPECS/SPEC-16-API-GATEWAY.md) for full specification.

// Crate-level lints
#![warn(missing_docs)]
#![allow(missing_docs)]
// TODO: Add documentation for all public items
// Phase 4 Exception: 37 excessive_nesting violations in middleware chain patterns
// These require architectural refactoring of RPC dispatch layers (middleware/*.rs)
// Reference: implementation_plan.md Phase 4 timeline estimates 4+ hours for this crate
#![allow(clippy::excessive_nesting)]
// Production code allows unwrap in controlled scenarios; tests use .unwrap() extensively
// CI/CD explicitly allows this to give team clarity on acceptable patterns
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![deny(unsafe_code)]

pub mod adapters;
pub mod domain;
pub mod ipc;
pub mod middleware;
pub mod ports;
pub mod rpc;
pub mod service;
pub mod router;
pub mod ws;

// Re-exports for public API (reduces cascade - use crate::X instead of crate::domain::X)
pub use domain::config::{CorsConfig, GatewayConfig, LimitsConfig, RateLimitConfig, TimeoutConfig};
pub use domain::correlation::CorrelationId;
pub use domain::error::{ApiError, ApiResult, GatewayError};
pub use domain::methods::{
    get_method_info, get_method_tier, get_method_timeout, is_method_supported, is_write_method,
    MethodInfo, MethodTier, SubscriptionType,
};
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
    #[allow(clippy::const_is_empty)]
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
