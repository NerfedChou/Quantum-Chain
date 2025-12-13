//! Timeout middleware per SPEC-16 Section 7.3.
//!
//! Applies per-method timeouts to prevent long-running requests.

use crate::TimeoutConfig;
use crate::ApiError;
use crate::get_method_timeout;
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tower::{Layer, Service};
use tracing::warn;

/// Timeout layer
#[derive(Clone)]
pub struct TimeoutLayer {
    config: Arc<TimeoutConfig>,
}

impl TimeoutLayer {
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = TimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimeoutService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Timeout service
#[derive(Clone)]
pub struct TimeoutService<S> {
    inner: S,
    config: Arc<TimeoutConfig>,
}

impl<S> Service<Request<Body>> for TimeoutService<S>
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
            // Determine timeout based on method
            let method_timeout = get_timeout_for_request(&req, &config);

            // Apply timeout
            match timeout(method_timeout, inner.call(req)).await {
                Ok(result) => result,
                Err(_) => {
                    warn!(timeout_ms = method_timeout.as_millis(), "Request timed out");
                    Ok(timeout_response(method_timeout))
                }
            }
        })
    }
}

/// Get timeout for a request based on method
fn get_timeout_for_request<B>(req: &Request<B>, config: &TimeoutConfig) -> Duration {
    // Check for method in custom header
    if let Some(method_header) = req.headers().get("x-rpc-method") {
        if let Ok(method) = method_header.to_str() {
            return get_method_timeout(method);
        }
    }

    // Check for method in query string
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some(method) = pair.strip_prefix("method=") {
                return get_method_timeout(method);
            }
        }
    }

    // Default timeout
    config.default
}

/// Create timeout response
fn timeout_response(timeout_duration: Duration) -> Response {
    let error = ApiError::timeout(format!(
        "Request exceeded {}s timeout",
        timeout_duration.as_secs()
    ));
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = StatusCode::GATEWAY_TIMEOUT;
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_timeout() {
        let config = TimeoutConfig::default();
        let req = Request::builder().body(Body::empty()).unwrap();
        let timeout = get_timeout_for_request(&req, &config);
        assert_eq!(timeout, config.default);
    }

    #[test]
    fn test_method_specific_timeout() {
        let config = TimeoutConfig::default();
        let req = Request::builder()
            .header("x-rpc-method", "eth_call")
            .body(Body::empty())
            .unwrap();

        let timeout = get_timeout_for_request(&req, &config);
        assert_eq!(timeout, Duration::from_secs(30)); // eth_call timeout
    }
}
