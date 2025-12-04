//! Tracing middleware for OpenTelemetry integration per SPEC-16 Section 8.
//!
//! Adds distributed tracing context to requests for the LGTM stack.

use axum::{body::Body, http::Request, response::Response};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use tracing::{info_span, Instrument, Span};

/// Tracing layer that creates spans for each request
#[derive(Clone, Default)]
pub struct TracingLayer;

impl TracingLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for TracingLayer {
    type Service = TracingService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TracingService { inner }
    }
}

/// Tracing service
#[derive(Clone)]
pub struct TracingService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for TracingService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();

        // Extract tracing context from headers
        let parent_context = extract_trace_context(&req);

        // Extract request info for span
        let method = req.method().clone();
        let uri = req.uri().clone();
        let rpc_method = req
            .headers()
            .get("x-rpc-method")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Create span
        let span = if let Some(rpc) = &rpc_method {
            info_span!(
                "api_request",
                http.method = %method,
                http.target = %uri.path(),
                rpc.method = %rpc,
                otel.kind = "server",
                otel.status_code = tracing::field::Empty,
            )
        } else {
            info_span!(
                "api_request",
                http.method = %method,
                http.target = %uri.path(),
                otel.kind = "server",
                otel.status_code = tracing::field::Empty,
            )
        };

        // Set parent context if available
        if let Some(parent) = parent_context {
            span.follows_from(parent);
        }

        Box::pin(
            async move {
                let result = inner.call(req).await;

                // Record status in span
                match &result {
                    Ok(response) => {
                        let status = response.status();
                        Span::current().record(
                            "otel.status_code",
                            if status.is_success() { "OK" } else { "ERROR" },
                        );
                    }
                    Err(_) => {
                        Span::current().record("otel.status_code", "ERROR");
                    }
                }

                result
            }
            .instrument(span),
        )
    }
}

/// Extract trace context from request headers (W3C Trace Context)
fn extract_trace_context<B>(req: &Request<B>) -> Option<Span> {
    // Look for traceparent header (W3C Trace Context)
    let traceparent = req.headers().get("traceparent")?.to_str().ok()?;

    // Parse traceparent: version-trace_id-parent_id-trace_flags
    let parts: Vec<&str> = traceparent.split('-').collect();
    if parts.len() != 4 {
        return None;
    }

    let trace_id = parts[1];
    let parent_id = parts[2];

    // Create a span that references the parent
    Some(info_span!(
        "parent_trace",
        trace_id = trace_id,
        parent_span_id = parent_id
    ))
}

/// Add trace context headers to outgoing requests
pub fn inject_trace_context(headers: &mut axum::http::HeaderMap) {
    // Get current trace context
    let span = Span::current();

    // In production, we'd use opentelemetry's propagator
    // For now, we'll add basic tracing headers
    if let Some(trace_id) = span.id() {
        let traceparent = format!(
            "00-{:032x}-{:016x}-01",
            trace_id.into_u64(),
            trace_id.into_u64()
        );
        if let Ok(value) = traceparent.parse() {
            headers.insert("traceparent", value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traceparent_parsing() {
        let req = Request::builder()
            .header(
                "traceparent",
                "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
            )
            .body(Body::empty())
            .unwrap();

        let parent = extract_trace_context(&req);
        assert!(parent.is_some());
    }

    #[test]
    fn test_invalid_traceparent() {
        let req = Request::builder()
            .header("traceparent", "invalid")
            .body(Body::empty())
            .unwrap();

        let parent = extract_trace_context(&req);
        assert!(parent.is_none());
    }

    #[test]
    fn test_no_traceparent() {
        let req = Request::builder().body(Body::empty()).unwrap();

        let parent = extract_trace_context(&req);
        assert!(parent.is_none());
    }
}
