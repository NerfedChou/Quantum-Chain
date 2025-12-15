//! Rate Limiter Adapters

use crate::ports::RateLimiter;
use std::collections::HashMap;
use std::sync::Mutex;

/// No-op rate limiter for testing (always allows).
#[derive(Debug, Default)]
pub struct NoOpRateLimiter;

impl NoOpRateLimiter {
    /// Create a no-op rate limiter.
    pub fn new() -> Self {
        Self
    }
}

impl RateLimiter for NoOpRateLimiter {
    fn check_rate(&self, _key: &[u8], _limit: u32, _window_secs: u64) -> bool {
        true // Always allow
    }
}

/// Sliding window rate limiter.
///
/// Tracks request counts per key within a time window.
pub struct SlidingWindowRateLimiter {
    /// Records: key -> (count, window_start_timestamp)
    records: Mutex<HashMap<Vec<u8>, (u32, u64)>>,
    /// Current time provider
    time_provider: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl std::fmt::Debug for SlidingWindowRateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlidingWindowRateLimiter")
            .field("records", &self.records)
            .field("time_provider", &"<closure>")
            .finish()
    }
}

impl SlidingWindowRateLimiter {
    /// Create a rate limiter with system time.
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            time_provider: Box::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            }),
        }
    }

    /// Create with custom time provider (for testing).
    pub fn with_time_provider<F>(provider: F) -> Self
    where
        F: Fn() -> u64 + Send + Sync + 'static,
    {
        Self {
            records: Mutex::new(HashMap::new()),
            time_provider: Box::new(provider),
        }
    }
}

impl Default for SlidingWindowRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter for SlidingWindowRateLimiter {
    fn check_rate(&self, key: &[u8], limit: u32, window_secs: u64) -> bool {
        let now = (self.time_provider)();
        let mut records = self.records.lock().unwrap();

        let key_vec = key.to_vec();
        let (count, window_start) = records.entry(key_vec).or_insert((0, now));

        // Check if window has expired
        if now >= *window_start + window_secs {
            // Reset window
            *window_start = now;
            *count = 1;
            return true;
        }

        // Within window
        if *count >= limit {
            return false; // Rate limited
        }

        *count += 1;
        true
    }
}
