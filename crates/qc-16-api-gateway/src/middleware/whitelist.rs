//! Method whitelist enforcement middleware per SPEC-16 Section 7.3.
//!
//! Rejects unknown methods at the gate - don't bother internal subsystems with invalid requests.
//! Also extracts method name into header for downstream middleware.

use crate::domain::methods::{get_method_info, is_method_supported};
use crate::ApiError;
use axum::{
    body::Body,
    http::{HeaderValue, Request, StatusCode},
    response::Response,
};
use std::sync::Arc;
use tower::{Layer, Service};
use tracing::warn;

/// Whitelist layer configuration
#[derive(Clone, Default)]
pub struct WhitelistConfig {
    /// Allow unknown methods (for forward compatibility)
    pub allow_unknown: bool,
    /// Additional allowed methods (not in registry)
    pub extra_methods: Vec<String>,
}

/// Whitelist enforcement layer
#[derive(Clone)]
pub struct WhitelistLayer {
    config: Arc<WhitelistConfig>,
}

impl WhitelistLayer {
    pub fn new(config: WhitelistConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub fn strict() -> Self {
        Self::new(WhitelistConfig::default())
    }
}

impl<S> Layer<S> for WhitelistLayer {
    type Service = WhitelistService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        WhitelistService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Whitelist enforcement service
#[derive(Clone)]
pub struct WhitelistService<S> {
    inner: S,
    config: Arc<WhitelistConfig>,
}

impl<S> Service<Request<Body>> for WhitelistService<S>
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
            // Only process POST requests (JSON-RPC)
            if req.method() != axum::http::Method::POST {
                return inner.call(req).await;
            }

            // Read body to extract method
            let (parts, body) = req.into_parts();

            // Read body bytes
            let body_bytes = match axum::body::to_bytes(body, 1024 * 1024).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    warn!(error = %e, "Failed to read request body for whitelist check");
                    return Ok(error_response(ApiError::internal(
                        "Failed to read request body",
                    )));
                }
            };

            // Try to extract method(s)
            let methods = extract_methods(&body_bytes);

            if methods.is_empty() {
                // Could not parse - let validation middleware handle it
                let req = Request::from_parts(parts, Body::from(body_bytes));
                return inner.call(req).await;
            }

            // Check all methods against whitelist
            for method in &methods {
                if !is_method_allowed(method, &config) {
                    warn!(method = method, "Rejected unknown method");
                    return Ok(method_not_found_response(method));
                }
            }

            // Set method header for downstream middleware (first method for single, or comma-separated)
            let mut parts = parts;
            if methods.len() == 1 {
                if let Ok(header_value) = HeaderValue::from_str(&methods[0]) {
                    parts.headers.insert("x-rpc-method", header_value);
                }
            } else {
                // Batch request
                if let Ok(header_value) = HeaderValue::from_str(&methods.join(",")) {
                    parts.headers.insert("x-rpc-methods", header_value);
                }
                // Set first method for rate limiting
                if let Ok(header_value) = HeaderValue::from_str(&methods[0]) {
                    parts.headers.insert("x-rpc-method", header_value);
                }
            }

            // Add method info to headers for timeout layer
            if methods.len() == 1 {
                if let Some(info) = get_method_info(&methods[0]) {
                    if let Ok(timeout) =
                        HeaderValue::from_str(&info.timeout().as_secs().to_string())
                    {
                        parts.headers.insert("x-rpc-timeout", timeout);
                    }
                    if let Ok(is_write) =
                        HeaderValue::from_str(if info.is_write() { "true" } else { "false" })
                    {
                        parts.headers.insert("x-rpc-write", is_write);
                    }
                }
            }

            // Reconstruct request and proceed
            let req = Request::from_parts(parts, Body::from(body_bytes));
            inner.call(req).await
        })
    }
}

/// Extract method names from JSON-RPC request body
fn extract_methods(body: &[u8]) -> Vec<String> {
    let mut methods = Vec::new();

    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) {
        match value {
            serde_json::Value::Object(obj) => {
                // Single request
                if let Some(serde_json::Value::String(method)) = obj.get("method") {
                    methods.push(method.clone());
                }
            }
            serde_json::Value::Array(arr) => {
                // Batch request
                for item in arr {
                    if let serde_json::Value::Object(obj) = item {
                        if let Some(serde_json::Value::String(method)) = obj.get("method") {
                            methods.push(method.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    methods
}

/// Check if method is allowed
fn is_method_allowed(method: &str, config: &WhitelistConfig) -> bool {
    // Check standard registry first
    if is_method_supported(method) {
        return true;
    }

    // Check extra methods
    if config.extra_methods.iter().any(|m| m == method) {
        return true;
    }

    // Allow unknown if configured
    config.allow_unknown
}

/// Create method not found response
fn method_not_found_response(method: &str) -> Response {
    let error = ApiError::method_not_found(method);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = StatusCode::OK; // JSON-RPC errors use 200 with error in body
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());

    response
}

/// Create error response
fn error_response(error: ApiError) -> Response {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = StatusCode::BAD_REQUEST;
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_method() {
        let body = br#"{"jsonrpc":"2.0","method":"eth_getBalance","params":[],"id":1}"#;
        let methods = extract_methods(body);
        assert_eq!(methods, vec!["eth_getBalance"]);
    }

    #[test]
    fn test_extract_batch_methods() {
        let body = br#"[{"jsonrpc":"2.0","method":"eth_getBalance","id":1},{"jsonrpc":"2.0","method":"eth_blockNumber","id":2}]"#;
        let methods = extract_methods(body);
        assert_eq!(methods, vec!["eth_getBalance", "eth_blockNumber"]);
    }

    #[test]
    fn test_method_allowed_known() {
        let config = WhitelistConfig::default();
        assert!(is_method_allowed("eth_getBalance", &config));
        assert!(is_method_allowed("eth_sendRawTransaction", &config));
    }

    #[test]
    fn test_method_rejected_unknown() {
        let config = WhitelistConfig::default();
        assert!(!is_method_allowed("eth_fakeMethod", &config));
        assert!(!is_method_allowed("malicious_method", &config));
    }

    #[test]
    fn test_extra_methods_allowed() {
        let config = WhitelistConfig {
            allow_unknown: false,
            extra_methods: vec!["custom_method".to_string()],
        };
        assert!(is_method_allowed("custom_method", &config));
        assert!(!is_method_allowed("other_method", &config));
    }

    #[test]
    fn test_allow_unknown() {
        let config = WhitelistConfig {
            allow_unknown: true,
            extra_methods: vec![],
        };
        assert!(is_method_allowed("any_method", &config));
    }
}
