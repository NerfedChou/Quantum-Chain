//! Domain types for the API Gateway.
//!
//! This module contains the core types, configuration, and error handling.
//! Note: Async infrastructure (pending requests) is in adapters layer.

pub mod config;
pub mod correlation;
pub mod error;
pub mod methods;
pub mod types;

// Re-exports for convenience
pub use config::{GatewayConfig, LimitsConfig};
pub use correlation::CorrelationId;
pub use error::{ApiError, ApiResult, GatewayError};
pub use methods::{get_method_info, get_method_tier, is_method_supported, MethodInfo, MethodTier};
pub use types::*;

// Re-export adapters pending (for backward compatibility)
pub use crate::adapters::pending::{PendingRequestStore, SubsystemResponse};
