//! Rate limiting middleware using token bucket algorithm per SPEC-16 Section 7.1.
//!
//! Implements per-IP rate limiting with configurable limits for reads and writes.
//! Write detection uses method registry for accuracy.

use crate::domain::config::RateLimitConfig;
use crate::domain::error::ApiError;
use crate::domain::methods::is_write_method;
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    response::Response,
};
use dashmap::DashMap;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::{Layer, Service};
use tracing::{debug, warn};

/// Token bucket entry for an IP address
struct TokenBucket {
    /// Read requests limiter
    read_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    /// Write requests limiter
    write_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    /// Last access time (for cleanup)
    last_access: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let read_quota = Quota::per_second(
            NonZeroU32::new(config.requests_per_second).unwrap_or(NonZeroU32::new(100).unwrap()),
        )
        .allow_burst(NonZeroU32::new(config.burst_size).unwrap_or(NonZeroU32::new(200).unwrap()));

        let write_quota = Quota::per_second(
            NonZeroU32::new(config.writes_per_second).unwrap_or(NonZeroU32::new(10).unwrap()),
        )
        .allow_burst(
            NonZeroU32::new(config.burst_size / 10).unwrap_or(NonZeroU32::new(20).unwrap()),
        );

        Self {
            read_limiter: RateLimiter::direct(read_quota),
            write_limiter: RateLimiter::direct(write_quota),
            last_access: Instant::now(),
        }
    }

    fn check_read(&mut self) -> Result<(), Duration> {
        self.last_access = Instant::now();
        match self.read_limiter.check() {
            Ok(_) => Ok(()),
            Err(not_until) => {
                let wait = not_until.wait_time_from(governor::clock::Clock::now(
                    &governor::clock::DefaultClock::default(),
                ));
                Err(wait)
            }
        }
    }

    fn check_write(&mut self) -> Result<(), Duration> {
        self.last_access = Instant::now();
        match self.write_limiter.check() {
            Ok(_) => Ok(()),
            Err(not_until) => {
                let wait = not_until.wait_time_from(governor::clock::Clock::now(
                    &governor::clock::DefaultClock::default(),
                ));
                Err(wait)
            }
        }
    }
}

/// Rate limiter state shared across requests
pub struct RateLimitState {
    /// Per-IP token buckets
    buckets: DashMap<IpAddr, TokenBucket>,
    /// Configuration
    config: RateLimitConfig,
}

impl RateLimitState {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: DashMap::new(),
            config,
        }
    }

    /// Check if request should be allowed
    pub fn check(&self, ip: IpAddr, is_write: bool) -> Result<(), Duration> {
        // Check whitelist
        if self.config.whitelist.contains(&ip) {
            return Ok(());
        }

        // Check if rate limiting is enabled
        if !self.config.enabled {
            return Ok(());
        }

        // Get or create bucket for this IP
        let mut bucket = self.buckets.entry(ip).or_insert_with(|| {
            debug!(ip = %ip, "Creating new rate limit bucket");
            TokenBucket::new(&self.config)
        });

        if is_write {
            bucket.check_write()
        } else {
            bucket.check_read()
        }
    }

    /// Clean up old buckets (call periodically)
    pub fn cleanup(&self, max_age: Duration) {
        let now = Instant::now();
        self.buckets.retain(|ip, bucket| {
            let age = now.duration_since(bucket.last_access);
            if age > max_age {
                debug!(ip = %ip, age_secs = age.as_secs(), "Removing stale rate limit bucket");
                false
            } else {
                true
            }
        });
    }

    /// Get number of tracked IPs
    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }
}

/// Rate limit layer
#[derive(Clone)]
pub struct RateLimitLayer {
    state: Arc<RateLimitState>,
}

impl RateLimitLayer {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            state: Arc::new(RateLimitState::new(config)),
        }
    }

    pub fn state(&self) -> Arc<RateLimitState> {
        Arc::clone(&self.state)
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            state: Arc::clone(&self.state),
        }
    }
}

/// Rate limit service
#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    state: Arc<RateLimitState>,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
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
        let state = Arc::clone(&self.state);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Extract client IP
            let ip = extract_client_ip(&req);

            // Determine if this is a write request by checking method from header
            let is_write = req
                .headers()
                .get("x-rpc-method")
                .and_then(|h| h.to_str().ok())
                .map(|m| is_write_method(m))
                .unwrap_or(false);

            // Check rate limit
            match state.check(ip, is_write) {
                Ok(()) => {
                    // Allowed - proceed with request
                    inner.call(req).await
                }
                Err(retry_after) => {
                    // Rate limited
                    let retry_ms = retry_after.as_millis() as u64;
                    warn!(
                        ip = %ip,
                        retry_after_ms = retry_ms,
                        is_write = is_write,
                        "Rate limit exceeded"
                    );

                    Ok(rate_limit_response(retry_ms))
                }
            }
        })
    }
}

/// Extract client IP from request
fn extract_client_ip<B>(req: &Request<B>) -> IpAddr {
    // Try X-Forwarded-For header first (for proxied requests)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // Take the first IP (original client)
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // Fall back to connection info
    if let Some(connect_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return connect_info.0.ip();
    }

    // Default to localhost if we can't determine IP
    IpAddr::from([127, 0, 0, 1])
}

/// Create rate limit exceeded response
fn rate_limit_response(retry_after_ms: u64) -> Response {
    let error = ApiError::rate_limited(retry_after_ms);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    response.headers_mut().insert(
        "Retry-After",
        ((retry_after_ms + 999) / 1000).to_string().parse().unwrap(),
    );

    response
}

/// Background task to clean up stale rate limit buckets
pub async fn cleanup_task(state: Arc<RateLimitState>, interval: Duration, max_age: Duration) {
    let mut cleanup_interval = tokio::time::interval(interval);
    cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        cleanup_interval.tick().await;
        state.cleanup(max_age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn test_config() -> RateLimitConfig {
        RateLimitConfig {
            requests_per_second: 10,
            writes_per_second: 2,
            burst_size: 20,
            enabled: true,
            whitelist: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))],
            window: Duration::from_secs(1),
        }
    }

    #[test]
    fn test_rate_limit_allows_within_limit() {
        let state = RateLimitState::new(test_config());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Should allow first requests
        for _ in 0..10 {
            assert!(state.check(ip, false).is_ok());
        }
    }

    #[test]
    fn test_rate_limit_blocks_over_limit() {
        let state = RateLimitState::new(test_config());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

        // Exhaust burst
        for _ in 0..25 {
            let _ = state.check(ip, false);
        }

        // Should be rate limited now
        let result = state.check(ip, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_whitelist_bypasses_limit() {
        let state = RateLimitState::new(test_config());
        let whitelisted_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        // Should always allow whitelisted IP
        for _ in 0..100 {
            assert!(state.check(whitelisted_ip, false).is_ok());
        }
    }

    #[test]
    fn test_disabled_rate_limiting() {
        let mut config = test_config();
        config.enabled = false;
        let state = RateLimitState::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

        // Should always allow when disabled
        for _ in 0..100 {
            assert!(state.check(ip, false).is_ok());
        }
    }

    #[test]
    fn test_write_vs_read_limits() {
        let state = RateLimitState::new(test_config());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4));

        // Writes have lower limit
        for _ in 0..5 {
            let _ = state.check(ip, true);
        }

        // Writes should be limited before reads
        let write_result = state.check(ip, true);
        assert!(write_result.is_err());

        // Reads should still have budget (separate bucket)
        // Note: With governor, the buckets are separate so this might still work
    }

    #[test]
    fn test_cleanup_removes_stale_buckets() {
        let state = RateLimitState::new(test_config());
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));

        let _ = state.check(ip, false);
        assert_eq!(state.bucket_count(), 1);

        // Cleanup with 0 duration should remove all
        state.cleanup(Duration::ZERO);
        assert_eq!(state.bucket_count(), 0);
    }
}
