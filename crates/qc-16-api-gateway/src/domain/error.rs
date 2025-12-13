//! API Gateway error types with JSON-RPC 2.0 error codes.
//!
//! Error codes follow Ethereum JSON-RPC spec and SPEC-16 Section 9.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Standard JSON-RPC 2.0 error codes
pub mod codes {
    // JSON-RPC 2.0 standard errors (-32700 to -32600)
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Server errors (-32000 to -32099)
    pub const SERVER_ERROR: i32 = -32000;
    pub const RESOURCE_NOT_FOUND: i32 = -32001;
    pub const RESOURCE_UNAVAILABLE: i32 = -32002;
    pub const TRANSACTION_REJECTED: i32 = -32003;
    pub const METHOD_NOT_SUPPORTED: i32 = -32004;
    pub const LIMIT_EXCEEDED: i32 = -32005;
    pub const TIMEOUT: i32 = -32006;

    // Ethereum specific errors (-32000 range, per EIP-1474)
    pub const UNAUTHORIZED: i32 = -32010;
    pub const ACTION_NOT_ALLOWED: i32 = -32011;
    pub const EXECUTION_ERROR: i32 = -32015;

    // Custom rate limit error
    pub const RATE_LIMITED: i32 = -32029;
}

/// API Gateway error with JSON-RPC code
#[derive(Debug, Clone)]
pub struct ApiError {
    /// JSON-RPC error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Optional additional data
    pub data: Option<serde_json::Value>,
}

impl ApiError {
    /// Create a new API error
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Create error with additional data
    pub fn with_data(code: i32, message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            code,
            message: message.into(),
            data: Some(data),
        }
    }

    // Standard JSON-RPC errors

    /// Parse error - invalid JSON
    pub fn parse_error(details: impl Into<String>) -> Self {
        Self::new(
            codes::PARSE_ERROR,
            format!("Parse error: {}", details.into()),
        )
    }

    /// Invalid request - not a valid JSON-RPC request
    pub fn invalid_request(details: impl Into<String>) -> Self {
        Self::new(
            codes::INVALID_REQUEST,
            format!("Invalid request: {}", details.into()),
        )
    }

    /// Method not found
    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            codes::METHOD_NOT_FOUND,
            format!("Method not found: {}", method),
        )
    }

    /// Invalid parameters
    pub fn invalid_params(details: impl Into<String>) -> Self {
        Self::new(
            codes::INVALID_PARAMS,
            format!("Invalid params: {}", details.into()),
        )
    }

    /// Internal error
    pub fn internal(details: impl Into<String>) -> Self {
        Self::new(
            codes::INTERNAL_ERROR,
            format!("Internal error: {}", details.into()),
        )
    }

    // Server errors

    /// Generic server error
    pub fn server_error(details: impl Into<String>) -> Self {
        Self::new(codes::SERVER_ERROR, details.into())
    }

    /// Resource not found (block, transaction, etc.)
    pub fn resource_not_found(resource: impl Into<String>) -> Self {
        Self::new(
            codes::RESOURCE_NOT_FOUND,
            format!("Resource not found: {}", resource.into()),
        )
    }

    /// Resource unavailable (syncing, etc.)
    pub fn resource_unavailable(details: impl Into<String>) -> Self {
        Self::new(
            codes::RESOURCE_UNAVAILABLE,
            format!("Resource unavailable: {}", details.into()),
        )
    }

    /// Transaction rejected
    pub fn transaction_rejected(reason: impl Into<String>) -> Self {
        Self::new(
            codes::TRANSACTION_REJECTED,
            format!("Transaction rejected: {}", reason.into()),
        )
    }

    /// Method not supported
    pub fn method_not_supported(method: &str) -> Self {
        Self::new(
            codes::METHOD_NOT_SUPPORTED,
            format!("Method not supported: {}", method),
        )
    }

    /// Limit exceeded (rate limit, batch size, etc.)
    pub fn limit_exceeded(limit: impl Into<String>) -> Self {
        Self::new(
            codes::LIMIT_EXCEEDED,
            format!("Limit exceeded: {}", limit.into()),
        )
    }

    /// Request timeout
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::new(
            codes::TIMEOUT,
            format!("Request timeout: {}", operation.into()),
        )
    }

    /// Unauthorized - missing or invalid auth
    pub fn unauthorized(details: impl Into<String>) -> Self {
        Self::new(
            codes::UNAUTHORIZED,
            format!("Unauthorized: {}", details.into()),
        )
    }

    /// Action not allowed (tier restriction)
    pub fn action_not_allowed(details: impl Into<String>) -> Self {
        Self::new(
            codes::ACTION_NOT_ALLOWED,
            format!("Action not allowed: {}", details.into()),
        )
    }

    /// Execution error (revert, out of gas, etc.)
    pub fn execution_error(details: impl Into<String>, data: Option<Vec<u8>>) -> Self {
        let mut error = Self::new(
            codes::EXECUTION_ERROR,
            format!("Execution reverted: {}", details.into()),
        );
        if let Some(revert_data) = data {
            error.data = Some(serde_json::json!({
                "data": format!("0x{}", hex::encode(revert_data))
            }));
        }
        error
    }

    /// Rate limited
    pub fn rate_limited(retry_after_ms: u64) -> Self {
        Self::with_data(
            codes::RATE_LIMITED,
            "Rate limit exceeded",
            serde_json::json!({
                "retry_after_ms": retry_after_ms
            }),
        )
    }

    /// Convert to jsonrpsee error (when jsonrpsee feature is enabled in Cargo.toml)
    pub fn into_jsonrpsee_error(self) -> (i32, String, Option<serde_json::Value>) {
        (self.code, self.message, self.data)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

impl Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ApiError", 3)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", &self.message)?;
        if let Some(ref data) = self.data {
            state.serialize_field("data", data)?;
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for ApiError {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ErrorHelper {
            code: i32,
            message: String,
            data: Option<serde_json::Value>,
        }

        let helper = ErrorHelper::deserialize(deserializer)?;
        Ok(ApiError {
            code: helper.code,
            message: helper.message,
            data: helper.data,
        })
    }
}

// Conversions from common error types

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        if e.is_syntax() || e.is_eof() {
            ApiError::parse_error(e.to_string())
        } else {
            ApiError::invalid_params(e.to_string())
        }
    }
}

impl From<hex::FromHexError> for ApiError {
    fn from(e: hex::FromHexError) -> Self {
        ApiError::invalid_params(format!("invalid hex: {}", e))
    }
}

/// Result type for API operations
pub type ApiResult<T> = Result<T, ApiError>;

/// Gateway-level errors (not JSON-RPC, internal use)
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// Configuration error
    #[error("configuration error: {0}")]
    Config(String),

    /// Server socket bind error
    #[error("server bind error: {0}")]
    Bind(String),

    /// IPC communication error
    #[error("IPC communication error: {0}")]
    Ipc(String),

    /// Target subsystem not available
    #[error("subsystem unavailable: {0}")]
    SubsystemUnavailable(String),

    /// Shutdown in progress
    #[error("shutdown in progress")]
    ShuttingDown,

    /// Internal server error
    #[error("internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = ApiError::method_not_found("eth_foo");
        assert_eq!(err.code, codes::METHOD_NOT_FOUND);
        assert!(err.message.contains("eth_foo"));
    }

    #[test]
    fn test_error_serialization() {
        let err = ApiError::invalid_params("missing 'to' field");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("-32602"));
        assert!(json.contains("missing 'to' field"));
    }

    #[test]
    fn test_error_with_data() {
        let err = ApiError::rate_limited(1000);
        assert!(err.data.is_some());
        let data = err.data.unwrap();
        assert_eq!(data["retry_after_ms"], 1000);
    }

    #[test]
    fn test_execution_error_with_revert() {
        let revert_data = vec![0x08, 0xc3, 0x79, 0xa0]; // Error(string) selector
        let err = ApiError::execution_error("Insufficient balance", Some(revert_data));
        assert_eq!(err.code, codes::EXECUTION_ERROR);
        assert!(err.data.is_some());
    }

    #[test]
    fn test_from_serde_error() {
        let json_err: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
        let api_err: ApiError = json_err.unwrap_err().into();
        assert_eq!(api_err.code, codes::PARSE_ERROR);
    }

    #[test]
    fn test_into_jsonrpsee_error() {
        let err = ApiError::internal("test");
        let (code, message, data) = err.into_jsonrpsee_error();
        assert_eq!(code, codes::INTERNAL_ERROR);
        assert!(message.contains("test"));
        assert!(data.is_none());
    }
}
