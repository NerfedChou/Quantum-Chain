//! # Rate Limiting
//!
//! Rate limiting configuration and utilities.
//!
//! ## Security
//!
//! Rate limiting is implemented at the API Gateway level.
//! These types define the configuration for rate limits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limit configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Time window duration
    pub window: Duration,
    /// Whether to apply per-IP limiting
    pub per_ip: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 1000,
            window: Duration::from_secs(60),
            per_ip: true,
        }
    }
}

impl RateLimitConfig {
    /// Create a new rate limit configuration.
    pub fn new(max_requests: u32, window: Duration, per_ip: bool) -> Self {
        Self {
            max_requests,
            window,
            per_ip,
        }
    }

    /// Create a strict rate limit (100 req/min).
    pub fn strict() -> Self {
        Self {
            max_requests: 100,
            window: Duration::from_secs(60),
            per_ip: true,
        }
    }

    /// Create a permissive rate limit (10000 req/min).
    pub fn permissive() -> Self {
        Self {
            max_requests: 10000,
            window: Duration::from_secs(60),
            per_ip: false,
        }
    }
}

/// Result of a rate limit check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request allowed
    Allowed,
    /// Request rate limited
    Limited {
        /// Seconds until rate limit resets
        retry_after: u64,
    },
}

impl RateLimitResult {
    /// Check if the request is allowed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Rate limiter implementation.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    state: Arc<Mutex<RateLimitState>>,
}

#[derive(Debug)]
struct RateLimitState {
    /// Global request count in current window
    global_count: u32,
    /// Window start time
    window_start: Instant,
    /// Per-IP counts (mocked for now as we don't have IP context yet)
    _ip_counts: HashMap<String, u32>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(RateLimitState {
                global_count: 0,
                window_start: Instant::now(),
                _ip_counts: HashMap::new(),
            })),
        }
    }

    /// Check if a request is allowed.
    ///
    /// TODO: Add IP address parameter when transport layer provides it.
    pub fn check(&self) -> RateLimitResult {
        let mut state = self.state.lock().unwrap();
        let now = Instant::now();

        // Reset window if expired
        if now.duration_since(state.window_start) >= self.config.window {
            state.global_count = 0;
            state.window_start = now;
        }

        if state.global_count >= self.config.max_requests {
            let elapsed = now.duration_since(state.window_start);
            let remaining = if elapsed < self.config.window {
                (self.config.window - elapsed).as_secs()
            } else {
                0
            };
            return RateLimitResult::Limited { retry_after: remaining };
        }

        state.global_count += 1;
        RateLimitResult::Allowed
    }
}
