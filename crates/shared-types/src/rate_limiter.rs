//! # Rate Limiter
//!
//! Token bucket rate limiter for protecting external API calls.
//!
//! ## Security
//!
//! Rate limiting prevents:
//! - DoS attacks from flooding with requests
//! - Resource exhaustion
//! - API quota abuse with external services

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Token bucket rate limiter.
///
/// # Algorithm
///
/// Uses the token bucket algorithm:
/// - Tokens are added at a fixed rate
/// - Each request consumes one token
/// - Requests are rejected when no tokens available
pub struct RateLimiter {
    /// Maximum tokens in bucket.
    capacity: u64,
    /// Tokens to add per second.
    refill_rate: u64,
    /// Current token count.
    tokens: AtomicU64,
    /// Last refill time.
    last_refill: std::sync::Mutex<Instant>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    ///
    /// # Parameters
    ///
    /// - `capacity`: Maximum burst size
    /// - `refill_rate`: Tokens per second
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            refill_rate,
            tokens: AtomicU64::new(capacity),
            last_refill: std::sync::Mutex::new(Instant::now()),
        }
    }

    /// Try to acquire a token.
    ///
    /// Returns `true` if request is allowed, `false` if rate limited.
    pub fn try_acquire(&self) -> bool {
        self.refill();

        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }

            if self
                .tokens
                .compare_exchange(current, current - 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    /// Refill tokens based on elapsed time.
    fn refill(&self) {
        let mut last = self.last_refill.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(*last);

        // Calculate tokens to add
        let tokens_to_add = (elapsed.as_secs_f64() * self.refill_rate as f64) as u64;

        if tokens_to_add > 0 {
            *last = now;

            loop {
                let current = self.tokens.load(Ordering::Relaxed);
                let new_value = (current + tokens_to_add).min(self.capacity);

                if self
                    .tokens
                    .compare_exchange(current, new_value, Ordering::SeqCst, Ordering::Relaxed)
                    .is_ok()
                {
                    break;
                }
            }
        }
    }

    /// Get current available tokens.
    pub fn available(&self) -> u64 {
        self.refill();
        self.tokens.load(Ordering::Relaxed)
    }

    /// Check if rate limited without consuming token.
    pub fn is_limited(&self) -> bool {
        self.available() == 0
    }
}

/// Pre-configured rate limiters for common use cases.
pub mod presets {
    use super::RateLimiter;

    /// External API calls (10 req/sec, burst 20).
    pub fn external_api() -> RateLimiter {
        RateLimiter::new(20, 10)
    }

    /// Light client sync (100 req/sec, burst 500).
    pub fn light_client_sync() -> RateLimiter {
        RateLimiter::new(500, 100)
    }

    /// Cross-chain proofs (5 req/sec, burst 10).
    pub fn cross_chain_proofs() -> RateLimiter {
        RateLimiter::new(10, 5)
    }

    /// Peer discovery (50 req/sec, burst 100).
    pub fn peer_discovery() -> RateLimiter {
        RateLimiter::new(100, 50)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_rate_limiter_allows_within_capacity() {
        let limiter = RateLimiter::new(5, 1);

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.try_acquire());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_capacity() {
        let limiter = RateLimiter::new(3, 1);

        // Exhaust tokens
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());

        // Next should fail
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_refills_over_time() {
        let limiter = RateLimiter::new(5, 100); // 100 tokens/sec

        // Exhaust all tokens
        for _ in 0..5 {
            limiter.try_acquire();
        }
        assert!(!limiter.try_acquire());

        // Wait for refill (100ms should add ~10 tokens, capped at 5)
        thread::sleep(Duration::from_millis(100));

        // Should be able to acquire again
        assert!(limiter.try_acquire());
    }

    #[test]
    fn test_rate_limiter_is_limited() {
        let limiter = RateLimiter::new(2, 0); // No refill

        assert!(!limiter.is_limited());
        limiter.try_acquire();
        limiter.try_acquire();
        assert!(limiter.is_limited());
    }

    #[test]
    fn test_presets() {
        let external = presets::external_api();
        assert_eq!(external.available(), 20);

        let lc = presets::light_client_sync();
        assert_eq!(lc.available(), 500);
    }
}
