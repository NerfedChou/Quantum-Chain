//! Pending Request Store - Async-to-Sync bridge per SPEC-16 Section 6.2.
//!
//! Maps correlation IDs to waiting HTTP/WebSocket requests for event bus responses.

use crate::domain::correlation::CorrelationId;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{debug, warn};

/// Response from subsystem
#[derive(Debug)]
pub struct SubsystemResponse {
    /// Correlation ID this response is for
    pub correlation_id: CorrelationId,
    /// Result (JSON-serializable)
    pub result: Result<serde_json::Value, ResponseError>,
    /// Response time
    pub response_time: Duration,
}

/// Error from subsystem
#[derive(Debug, Clone)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// A pending request waiting for response
struct PendingRequest {
    /// Channel to send response
    sender: oneshot::Sender<SubsystemResponse>,
    /// When request was created
    created_at: Instant,
    /// Method name (for logging)
    method: String,
    /// Timeout for this request
    timeout: Duration,
}

/// Statistics for pending request store
#[derive(Debug, Default)]
pub struct PendingStats {
    /// Total requests registered
    pub total_registered: AtomicU64,
    /// Total requests completed
    pub total_completed: AtomicU64,
    /// Total requests timed out
    pub total_timeouts: AtomicU64,
    /// Total requests cancelled (dropped)
    pub total_cancelled: AtomicU64,
}

/// Pending request store for async-to-sync bridging.
///
/// Flow:
/// 1. RPC handler generates CorrelationId
/// 2. Handler calls `register()` to get a oneshot receiver
/// 3. Handler sends IPC request with CorrelationId
/// 4. Response listener receives response and calls `complete()`
/// 5. Handler awaits the receiver or times out
pub struct PendingRequestStore {
    /// Map of correlation ID to pending request
    pending: DashMap<CorrelationId, PendingRequest>,
    /// Default timeout
    default_timeout: Duration,
    /// Statistics
    stats: Arc<PendingStats>,
}

impl PendingRequestStore {
    /// Create a new pending request store
    pub fn new(default_timeout: Duration) -> Self {
        Self {
            pending: DashMap::new(),
            default_timeout,
            stats: Arc::new(PendingStats::default()),
        }
    }

    /// Register a pending request and get a receiver for the response.
    ///
    /// Returns the correlation ID and a receiver that will receive the response.
    pub fn register(
        &self,
        method: &str,
        timeout: Option<Duration>,
    ) -> (CorrelationId, oneshot::Receiver<SubsystemResponse>) {
        let correlation_id = CorrelationId::new();
        let (tx, rx) = oneshot::channel();

        let request = PendingRequest {
            sender: tx,
            created_at: Instant::now(),
            method: method.to_string(),
            timeout: timeout.unwrap_or(self.default_timeout),
        };

        self.pending.insert(correlation_id, request);
        self.stats.total_registered.fetch_add(1, Ordering::Relaxed);

        debug!(
            correlation_id = %correlation_id,
            method = method,
            "Registered pending request"
        );

        (correlation_id, rx)
    }

    /// Complete a pending request with a response.
    ///
    /// Returns true if the request was found and completed, false if not found or already completed.
    pub fn complete(
        &self,
        correlation_id: CorrelationId,
        result: Result<serde_json::Value, ResponseError>,
    ) -> bool {
        if let Some((_, pending)) = self.pending.remove(&correlation_id) {
            let response_time = pending.created_at.elapsed();

            let response = SubsystemResponse {
                correlation_id,
                result,
                response_time,
            };

            match pending.sender.send(response) {
                Ok(()) => {
                    self.stats.total_completed.fetch_add(1, Ordering::Relaxed);
                    debug!(
                        correlation_id = %correlation_id,
                        method = pending.method,
                        response_time_ms = response_time.as_millis(),
                        "Completed pending request"
                    );
                    true
                }
                Err(_) => {
                    // Receiver was dropped (request cancelled)
                    self.stats.total_cancelled.fetch_add(1, Ordering::Relaxed);
                    debug!(
                        correlation_id = %correlation_id,
                        method = pending.method,
                        "Pending request receiver dropped"
                    );
                    false
                }
            }
        } else {
            warn!(
                correlation_id = %correlation_id,
                "Response for unknown or expired correlation ID"
            );
            false
        }
    }

    /// Remove expired requests (TTL cleanup).
    ///
    /// Returns the number of requests removed.
    pub fn remove_expired(&self) -> usize {
        let now = Instant::now();
        let mut removed = 0;

        self.pending.retain(|id, request| {
            let elapsed = now.duration_since(request.created_at);
            if elapsed > request.timeout {
                warn!(
                    correlation_id = %id,
                    method = request.method,
                    elapsed_ms = elapsed.as_millis(),
                    timeout_ms = request.timeout.as_millis(),
                    "Removing expired pending request"
                );
                self.stats.total_timeouts.fetch_add(1, Ordering::Relaxed);
                removed += 1;
                false // Remove
            } else {
                true // Keep
            }
        });

        removed
    }

    /// Get number of currently pending requests
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get statistics
    pub fn stats(&self) -> &PendingStats {
        &self.stats
    }

    /// Check if a correlation ID is pending
    pub fn is_pending(&self, correlation_id: &CorrelationId) -> bool {
        self.pending.contains_key(correlation_id)
    }

    /// Cancel a pending request
    pub fn cancel(&self, correlation_id: &CorrelationId) -> bool {
        if let Some((_, _)) = self.pending.remove(correlation_id) {
            self.stats.total_cancelled.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Get metrics for IPC handler (admin panel support)
    pub fn get_metrics(&self) -> crate::ipc::handler::IpcHandlerMetrics {
        use crate::ipc::handler::IpcHandlerMetrics;
        
        let stats = self.stats();
        let total_sent = stats.total_registered.load(Ordering::Relaxed);
        let total_received = stats.total_completed.load(Ordering::Relaxed);
        let total_timeouts = stats.total_timeouts.load(Ordering::Relaxed);
        let total_errors = stats.total_cancelled.load(Ordering::Relaxed);

        IpcHandlerMetrics {
            total_sent,
            total_received,
            total_errors,
            total_timeouts,
            requests_per_sec: 0.0, // Would need time-windowed tracking
            errors_per_sec: 0.0,
            p50_latency_ms: 0,     // Would need latency histogram
            p99_latency_ms: 0,
            by_subsystem: std::collections::HashMap::new(),
        }
    }
}

/// Background task to clean up expired requests
pub async fn cleanup_task(store: Arc<PendingRequestStore>, interval: Duration) {
    let mut cleanup_interval = tokio::time::interval(interval);
    cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        cleanup_interval.tick().await;
        let removed = store.remove_expired();
        if removed > 0 {
            debug!(removed = removed, "Cleaned up expired pending requests");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_complete() {
        let store = PendingRequestStore::new(Duration::from_secs(30));

        let (correlation_id, rx) = store.register("eth_getBalance", None);
        assert!(store.is_pending(&correlation_id));
        assert_eq!(store.pending_count(), 1);

        let result = serde_json::json!("0x1234");
        assert!(store.complete(correlation_id, Ok(result.clone())));

        let response = rx.await.unwrap();
        assert_eq!(response.correlation_id, correlation_id);
        assert_eq!(response.result.unwrap(), result);
        assert_eq!(store.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_complete_unknown_id() {
        let store = PendingRequestStore::new(Duration::from_secs(30));
        let unknown_id = CorrelationId::new();

        assert!(!store.complete(unknown_id, Ok(serde_json::json!(null))));
    }

    #[tokio::test]
    async fn test_remove_expired() {
        let store = PendingRequestStore::new(Duration::from_millis(10));

        let (id1, _rx1) = store.register("eth_getBalance", None);
        let (id2, _rx2) = store.register("eth_getBalance", None);

        assert_eq!(store.pending_count(), 2);

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(50)).await;

        let removed = store.remove_expired();
        assert_eq!(removed, 2);
        assert_eq!(store.pending_count(), 0);
        assert!(!store.is_pending(&id1));
        assert!(!store.is_pending(&id2));
    }

    #[tokio::test]
    async fn test_cancel() {
        let store = PendingRequestStore::new(Duration::from_secs(30));

        let (correlation_id, _rx) = store.register("eth_getBalance", None);
        assert!(store.is_pending(&correlation_id));

        assert!(store.cancel(&correlation_id));
        assert!(!store.is_pending(&correlation_id));

        // Cancel again should return false
        assert!(!store.cancel(&correlation_id));
    }

    #[tokio::test]
    async fn test_stats() {
        let store = PendingRequestStore::new(Duration::from_millis(10));

        let (id1, _rx1) = store.register("eth_getBalance", None);
        let (id2, _rx2) = store.register("eth_getBlock", None);

        assert_eq!(store.stats().total_registered.load(Ordering::Relaxed), 2);

        store.complete(id1, Ok(serde_json::json!(null)));
        assert_eq!(store.stats().total_completed.load(Ordering::Relaxed), 1);

        store.cancel(&id2);
        assert_eq!(store.stats().total_cancelled.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_custom_timeout() {
        let store = PendingRequestStore::new(Duration::from_secs(30));

        // Register with short timeout
        let (_id, _rx) = store.register("eth_getBalance", Some(Duration::from_millis(5)));

        assert_eq!(store.pending_count(), 1);

        // Wait for custom timeout
        tokio::time::sleep(Duration::from_millis(20)).await;

        let removed = store.remove_expired();
        assert_eq!(removed, 1);
    }
}
