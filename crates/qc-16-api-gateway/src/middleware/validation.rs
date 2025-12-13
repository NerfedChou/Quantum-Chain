//! Request validation middleware per SPEC-16 Section 7.2.
//!
//! Validates request size, batch limits, and JSON-RPC structure.

use crate::ApiError;
use crate::LimitsConfig;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use bytes::Bytes;
use std::sync::Arc;
use tower::{Layer, Service};
use tracing::warn;

/// Validation layer configuration
#[derive(Clone)]
pub struct ValidationLayer {
    config: Arc<LimitsConfig>,
}

impl ValidationLayer {
    pub fn new(config: LimitsConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for ValidationLayer {
    type Service = ValidationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ValidationService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Validation service
#[derive(Clone)]
pub struct ValidationService<S> {
    inner: S,
    config: Arc<LimitsConfig>,
}

impl<S> Service<Request<Body>> for ValidationService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let config = Arc::clone(&self.config);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check content-length header first (fast path)
            if let Some(content_length) = req.headers().get("content-length") {
                if let Ok(len_str) = content_length.to_str() {
                    if let Ok(len) = len_str.parse::<usize>() {
                        if len > config.max_request_size {
                            warn!(
                                size = len,
                                max = config.max_request_size,
                                "Request too large (from header)"
                            );
                            return Ok(error_response(ApiError::limit_exceeded(format!(
                                "Request size {} exceeds limit {}",
                                len, config.max_request_size
                            ))));
                        }
                    }
                }
            }

            // For POST requests, we need to validate the body
            if req.method() == axum::http::Method::POST {
                // Collect body with size limit
                let (parts, body) = req.into_parts();

                // Read body with limit
                let body_bytes = match read_body_with_limit(body, config.max_request_size).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!(error = %e, "Failed to read request body");
                        return Ok(error_response(e));
                    }
                };

                // Validate JSON-RPC structure
                if let Err(e) = validate_jsonrpc(&body_bytes, &config) {
                    warn!(error = %e, "Invalid JSON-RPC request");
                    return Ok(error_response(e));
                }

                // Reconstruct request with validated body
                let req = Request::from_parts(parts, Body::from(body_bytes));
                inner.call(req).await
            } else {
                // Non-POST requests pass through
                inner.call(req).await
            }
        })
    }
}

/// Read body with size limit
async fn read_body_with_limit(body: Body, max_size: usize) -> Result<Bytes, ApiError> {
    use axum::body::to_bytes;

    let bytes = to_bytes(body, max_size)
        .await
        .map_err(|e| ApiError::limit_exceeded(format!("Failed to read body: {}", e)))?;

    Ok(bytes)
}

/// Validate JSON-RPC request structure
///
/// Exported for integration testing.
pub fn validate_jsonrpc(body: &[u8], config: &LimitsConfig) -> Result<(), ApiError> {
    // Try to parse as JSON
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|e| ApiError::parse_error(e.to_string()))?;

    match value {
        serde_json::Value::Object(obj) => {
            // Single request
            validate_single_request(&obj)?;
        }
        serde_json::Value::Array(arr) => {
            // Batch request
            if arr.is_empty() {
                return Err(ApiError::invalid_request("Empty batch request"));
            }

            if arr.len() > config.max_batch_size {
                return Err(ApiError::limit_exceeded(format!(
                    "Batch size {} exceeds limit {}",
                    arr.len(),
                    config.max_batch_size
                )));
            }

            for (idx, item) in arr.iter().enumerate() {
                if let serde_json::Value::Object(obj) = item {
                    validate_single_request(obj).map_err(|e| {
                        ApiError::invalid_request(format!("Batch item {}: {}", idx, e.message))
                    })?;
                } else {
                    return Err(ApiError::invalid_request(format!(
                        "Batch item {} is not an object",
                        idx
                    )));
                }
            }
        }
        _ => {
            return Err(ApiError::invalid_request(
                "Request must be an object or array",
            ));
        }
    }

    Ok(())
}

/// Validate a single JSON-RPC request object
fn validate_single_request(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), ApiError> {
    // Check jsonrpc version
    match obj.get("jsonrpc") {
        Some(serde_json::Value::String(v)) if v == "2.0" => {}
        Some(_) => {
            return Err(ApiError::invalid_request("jsonrpc must be \"2.0\""));
        }
        None => {
            return Err(ApiError::invalid_request("Missing jsonrpc field"));
        }
    }

    // Check method exists and is a string
    match obj.get("method") {
        Some(serde_json::Value::String(method)) => {
            if method.is_empty() {
                return Err(ApiError::invalid_request("Method cannot be empty"));
            }
            // Method name length limit
            if method.len() > 256 {
                return Err(ApiError::invalid_request("Method name too long"));
            }
        }
        Some(_) => {
            return Err(ApiError::invalid_request("method must be a string"));
        }
        None => {
            return Err(ApiError::invalid_request("Missing method field"));
        }
    }

    // id is optional for notifications, but if present must be string/number/null
    if let Some(id) = obj.get("id") {
        match id {
            serde_json::Value::String(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::Null => {}
            _ => {
                return Err(ApiError::invalid_request(
                    "id must be string, number, or null",
                ));
            }
        }
    }

    // params is optional, but if present must be array or object
    if let Some(params) = obj.get("params") {
        match params {
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {}
            _ => {
                return Err(ApiError::invalid_request("params must be array or object"));
            }
        }
    }

    Ok(())
}

/// Create error response
fn error_response(error: ApiError) -> Response {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let status = match error.code {
        -32700 => StatusCode::BAD_REQUEST,       // Parse error
        -32600 => StatusCode::BAD_REQUEST,       // Invalid request
        -32005 => StatusCode::PAYLOAD_TOO_LARGE, // Limit exceeded
        _ => StatusCode::BAD_REQUEST,
    };

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LimitsConfig {
        LimitsConfig {
            max_request_size: 1024,
            max_batch_size: 10,
            max_response_size: 1024,
            max_log_block_range: 1000,
            max_log_results: 1000,
        }
    }

    #[test]
    fn test_valid_single_request() {
        let body = br#"{"jsonrpc":"2.0","method":"eth_blockNumber","id":1}"#;
        assert!(validate_jsonrpc(body, &test_config()).is_ok());
    }

    #[test]
    fn test_valid_request_with_params() {
        let body =
            br#"{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x1234","latest"],"id":1}"#;
        assert!(validate_jsonrpc(body, &test_config()).is_ok());
    }

    #[test]
    fn test_valid_batch_request() {
        let body = br#"[{"jsonrpc":"2.0","method":"eth_blockNumber","id":1},{"jsonrpc":"2.0","method":"eth_chainId","id":2}]"#;
        assert!(validate_jsonrpc(body, &test_config()).is_ok());
    }

    #[test]
    fn test_missing_jsonrpc_field() {
        let body = br#"{"method":"eth_blockNumber","id":1}"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("jsonrpc"));
    }

    #[test]
    fn test_wrong_jsonrpc_version() {
        let body = br#"{"jsonrpc":"1.0","method":"eth_blockNumber","id":1}"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_method() {
        let body = br#"{"jsonrpc":"2.0","id":1}"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("method"));
    }

    #[test]
    fn test_empty_batch() {
        let body = br#"[]"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("Empty batch"));
    }

    #[test]
    fn test_batch_too_large() {
        let mut requests = Vec::new();
        for i in 0..15 {
            requests.push(format!(
                r#"{{"jsonrpc":"2.0","method":"eth_blockNumber","id":{}}}"#,
                i
            ));
        }
        let body = format!("[{}]", requests.join(","));

        let result = validate_jsonrpc(body.as_bytes(), &test_config());
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("Batch size"));
    }

    #[test]
    fn test_invalid_json() {
        let body = br#"{"jsonrpc":"2.0","method":"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
    }

    #[test]
    fn test_notification_no_id() {
        // Notifications are valid JSON-RPC without id
        let body = br#"{"jsonrpc":"2.0","method":"eth_subscribe","params":["newHeads"]}"#;
        assert!(validate_jsonrpc(body, &test_config()).is_ok());
    }

    #[test]
    fn test_invalid_id_type() {
        let body = br#"{"jsonrpc":"2.0","method":"eth_blockNumber","id":[]}"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_params_type() {
        let body = br#"{"jsonrpc":"2.0","method":"eth_blockNumber","params":"invalid","id":1}"#;
        let result = validate_jsonrpc(body, &test_config());
        assert!(result.is_err());
    }
}
