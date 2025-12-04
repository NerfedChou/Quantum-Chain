//! Prometheus metrics middleware per SPEC-16 Section 8.
//!
//! Exposes metrics for monitoring via Grafana/Prometheus.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// API Gateway metrics
#[derive(Default)]
pub struct GatewayMetrics {
    // Request counters
    pub requests_total: AtomicU64,
    pub requests_success: AtomicU64,
    pub requests_error: AtomicU64,

    // Write request counters
    pub write_requests_total: AtomicU64,

    // Rate limit counters
    pub rate_limit_rejected: AtomicU64,

    // WebSocket counters
    pub websocket_connections: AtomicU64,
    pub websocket_subscriptions: AtomicU64,
    pub websocket_messages_sent: AtomicU64,

    // Pending request counters
    pub pending_requests: AtomicU64,
    pub pending_timeouts: AtomicU64,

    // Latency tracking (simplified - in production use histograms)
    pub total_latency_ms: AtomicU64,
    pub request_count_for_latency: AtomicU64,
}

impl GatewayMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a request
    pub fn record_request(&self, success: bool, is_write: bool, latency_ms: u64) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);

        if success {
            self.requests_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.requests_error.fetch_add(1, Ordering::Relaxed);
        }

        if is_write {
            self.write_requests_total.fetch_add(1, Ordering::Relaxed);
        }

        self.total_latency_ms
            .fetch_add(latency_ms, Ordering::Relaxed);
        self.request_count_for_latency
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record rate limit rejection
    pub fn record_rate_limit_rejection(&self) {
        self.rate_limit_rejected.fetch_add(1, Ordering::Relaxed);
    }

    /// Record WebSocket connection
    pub fn record_ws_connect(&self) {
        self.websocket_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record WebSocket disconnection
    pub fn record_ws_disconnect(&self) {
        self.websocket_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record WebSocket subscription
    pub fn record_ws_subscribe(&self) {
        self.websocket_subscriptions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record WebSocket unsubscription
    pub fn record_ws_unsubscribe(&self) {
        self.websocket_subscriptions.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record WebSocket message sent
    pub fn record_ws_message(&self) {
        self.websocket_messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Record pending request created
    pub fn record_pending_created(&self) {
        self.pending_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Record pending request completed
    pub fn record_pending_completed(&self) {
        self.pending_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record pending request timeout
    pub fn record_pending_timeout(&self) {
        self.pending_timeouts.fetch_add(1, Ordering::Relaxed);
        self.pending_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get average latency in ms
    pub fn average_latency_ms(&self) -> f64 {
        let total = self.total_latency_ms.load(Ordering::Relaxed);
        let count = self.request_count_for_latency.load(Ordering::Relaxed);
        if count == 0 {
            0.0
        } else {
            total as f64 / count as f64
        }
    }

    /// Export metrics in Prometheus format
    #[cfg(feature = "metrics")]
    pub fn to_prometheus(&self) -> String {
        let mut output = String::new();

        // Request counters
        output.push_str(&format!(
            "# HELP api_gateway_requests_total Total number of API requests\n\
             # TYPE api_gateway_requests_total counter\n\
             api_gateway_requests_total {}\n",
            self.requests_total.load(Ordering::Relaxed)
        ));

        output.push_str(&format!(
            "# HELP api_gateway_requests_success_total Successful requests\n\
             # TYPE api_gateway_requests_success_total counter\n\
             api_gateway_requests_success_total {}\n",
            self.requests_success.load(Ordering::Relaxed)
        ));

        output.push_str(&format!(
            "# HELP api_gateway_requests_error_total Failed requests\n\
             # TYPE api_gateway_requests_error_total counter\n\
             api_gateway_requests_error_total {}\n",
            self.requests_error.load(Ordering::Relaxed)
        ));

        output.push_str(&format!(
            "# HELP api_gateway_write_requests_total Write requests (sendRawTransaction)\n\
             # TYPE api_gateway_write_requests_total counter\n\
             api_gateway_write_requests_total {}\n",
            self.write_requests_total.load(Ordering::Relaxed)
        ));

        // Rate limiting
        output.push_str(&format!(
            "# HELP api_gateway_rate_limit_rejected_total Rate limited requests\n\
             # TYPE api_gateway_rate_limit_rejected_total counter\n\
             api_gateway_rate_limit_rejected_total {}\n",
            self.rate_limit_rejected.load(Ordering::Relaxed)
        ));

        // WebSocket
        output.push_str(&format!(
            "# HELP api_gateway_websocket_connections Active WebSocket connections\n\
             # TYPE api_gateway_websocket_connections gauge\n\
             api_gateway_websocket_connections {}\n",
            self.websocket_connections.load(Ordering::Relaxed)
        ));

        output.push_str(&format!(
            "# HELP api_gateway_websocket_subscriptions Active subscriptions\n\
             # TYPE api_gateway_websocket_subscriptions gauge\n\
             api_gateway_websocket_subscriptions {}\n",
            self.websocket_subscriptions.load(Ordering::Relaxed)
        ));

        // Pending requests
        output.push_str(&format!(
            "# HELP api_gateway_pending_requests Current pending requests\n\
             # TYPE api_gateway_pending_requests gauge\n\
             api_gateway_pending_requests {}\n",
            self.pending_requests.load(Ordering::Relaxed)
        ));

        output.push_str(&format!(
            "# HELP api_gateway_pending_timeouts_total Timed out requests\n\
             # TYPE api_gateway_pending_timeouts_total counter\n\
             api_gateway_pending_timeouts_total {}\n",
            self.pending_timeouts.load(Ordering::Relaxed)
        ));

        // Latency
        output.push_str(&format!(
            "# HELP api_gateway_average_latency_ms Average request latency\n\
             # TYPE api_gateway_average_latency_ms gauge\n\
             api_gateway_average_latency_ms {:.2}\n",
            self.average_latency_ms()
        ));

        output
    }

    /// Export metrics as JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "requests": {
                "total": self.requests_total.load(Ordering::Relaxed),
                "success": self.requests_success.load(Ordering::Relaxed),
                "error": self.requests_error.load(Ordering::Relaxed),
                "writes": self.write_requests_total.load(Ordering::Relaxed),
            },
            "rate_limiting": {
                "rejected": self.rate_limit_rejected.load(Ordering::Relaxed),
            },
            "websocket": {
                "connections": self.websocket_connections.load(Ordering::Relaxed),
                "subscriptions": self.websocket_subscriptions.load(Ordering::Relaxed),
                "messages_sent": self.websocket_messages_sent.load(Ordering::Relaxed),
            },
            "pending": {
                "current": self.pending_requests.load(Ordering::Relaxed),
                "timeouts": self.pending_timeouts.load(Ordering::Relaxed),
            },
            "latency": {
                "average_ms": self.average_latency_ms(),
            }
        })
    }
}

/// Request timing helper
pub struct RequestTimer {
    start: Instant,
    metrics: Arc<GatewayMetrics>,
    is_write: bool,
}

impl RequestTimer {
    pub fn new(metrics: Arc<GatewayMetrics>, is_write: bool) -> Self {
        Self {
            start: Instant::now(),
            metrics,
            is_write,
        }
    }

    pub fn finish(self, success: bool) {
        let latency_ms = self.start.elapsed().as_millis() as u64;
        self.metrics
            .record_request(success, self.is_write, latency_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = GatewayMetrics::new();

        metrics.record_request(true, false, 100);
        metrics.record_request(true, false, 200);
        metrics.record_request(false, true, 50);

        assert_eq!(metrics.requests_total.load(Ordering::Relaxed), 3);
        assert_eq!(metrics.requests_success.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.requests_error.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.write_requests_total.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_average_latency() {
        let metrics = GatewayMetrics::new();

        metrics.record_request(true, false, 100);
        metrics.record_request(true, false, 200);
        metrics.record_request(true, false, 300);

        assert!((metrics.average_latency_ms() - 200.0).abs() < 0.01);
    }

    #[test]
    fn test_websocket_metrics() {
        let metrics = GatewayMetrics::new();

        metrics.record_ws_connect();
        metrics.record_ws_connect();
        metrics.record_ws_subscribe();
        metrics.record_ws_subscribe();
        metrics.record_ws_subscribe();

        assert_eq!(metrics.websocket_connections.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.websocket_subscriptions.load(Ordering::Relaxed), 3);

        metrics.record_ws_disconnect();
        metrics.record_ws_unsubscribe();

        assert_eq!(metrics.websocket_connections.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.websocket_subscriptions.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_json_export() {
        let metrics = GatewayMetrics::new();
        metrics.record_request(true, false, 100);

        let json = metrics.to_json();
        assert_eq!(json["requests"]["total"], 1);
        assert_eq!(json["requests"]["success"], 1);
    }
}
