//! # Time-Bounded Nonce Cache
//!
//! Implements replay attack prevention as specified in Architecture.md v2.1.
//!
//! ## Security Design
//!
//! - Nonces are valid only within the message timestamp window (60s past, 10s future)
//! - Nonces are garbage-collected after the validity window expires
//! - This bounds memory usage while preventing replay attacks

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

/// Errors from nonce cache operations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NonceError {
    /// The nonce has already been used (replay attack).
    #[error("Nonce {nonce} has already been used (replay attack)")]
    NonceReused { nonce: Uuid },

    /// The message timestamp is too old.
    #[error("Message timestamp {timestamp} is too old (threshold: {threshold})")]
    MessageTooOld { timestamp: u64, threshold: u64 },

    /// The message timestamp is in the future.
    #[error("Message timestamp {timestamp} is in the future (threshold: {threshold})")]
    MessageFromFuture { timestamp: u64, threshold: u64 },
}

/// Time-bounded cache for replay prevention.
///
/// Per Architecture.md v2.1:
/// - Timestamp window: now - 60s to now + 10s
/// - Nonce validity: 120s (2x the timestamp window)
/// - Garbage collection: Every 10s
pub struct TimeBoundedNonceCache {
    /// Map of nonce -> timestamp when nonce was first seen.
    cache: HashMap<Uuid, u64>,

    /// Nonce validity window in seconds (default: 120s = 2x message window).
    validity_window_secs: u64,

    /// Last garbage collection timestamp.
    last_gc: u64,

    /// Garbage collection interval in seconds.
    gc_interval_secs: u64,
}

impl TimeBoundedNonceCache {
    /// Default validity window: 2x the 60s message window.
    pub const DEFAULT_VALIDITY_WINDOW: u64 = 120;

    /// Default garbage collection interval.
    pub const DEFAULT_GC_INTERVAL: u64 = 10;

    /// Maximum past age for valid timestamps.
    pub const MAX_AGE: u64 = 60;

    /// Maximum future skew for valid timestamps.
    pub const MAX_FUTURE_SKEW: u64 = 10;

    /// Create a new nonce cache with default settings.
    #[must_use]
    pub fn new() -> Self {
        let now = Self::current_timestamp();
        Self {
            cache: HashMap::new(),
            validity_window_secs: Self::DEFAULT_VALIDITY_WINDOW,
            last_gc: now,
            gc_interval_secs: Self::DEFAULT_GC_INTERVAL,
        }
    }

    /// Create a nonce cache with custom settings.
    #[must_use]
    pub fn with_config(validity_window_secs: u64, gc_interval_secs: u64) -> Self {
        let now = Self::current_timestamp();
        Self {
            cache: HashMap::new(),
            validity_window_secs,
            last_gc: now,
            gc_interval_secs,
        }
    }

    /// Validate timestamp and check/add nonce atomically.
    ///
    /// # Steps (per Architecture.md)
    ///
    /// 1. **TIMESTAMP CHECK FIRST** - Reject messages outside valid window
    /// 2. **NONCE CHECK** - Reject if nonce already seen
    /// 3. **ADD NONCE** - Store nonce with timestamp for later expiration
    ///
    /// # Errors
    ///
    /// - `NonceError::MessageTooOld` - Timestamp older than 60s
    /// - `NonceError::MessageFromFuture` - Timestamp more than 10s in future
    /// - `NonceError::NonceReused` - Nonce has been seen before
    pub fn validate_and_add(&mut self, nonce: Uuid, timestamp: u64) -> Result<(), NonceError> {
        let now = Self::current_timestamp();

        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 1: TIMESTAMP CHECK (MUST BE FIRST!)                     ║
        // ║                                                               ║
        // ║  Reject messages outside the valid time window BEFORE any    ║
        // ║  other processing. This bounds all subsequent operations.    ║
        // ╚═══════════════════════════════════════════════════════════════╝

        let min_valid_timestamp = now.saturating_sub(Self::MAX_AGE);
        let max_valid_timestamp = now.saturating_add(Self::MAX_FUTURE_SKEW);

        if timestamp < min_valid_timestamp {
            return Err(NonceError::MessageTooOld {
                timestamp,
                threshold: min_valid_timestamp,
            });
        }

        if timestamp > max_valid_timestamp {
            return Err(NonceError::MessageFromFuture {
                timestamp,
                threshold: max_valid_timestamp,
            });
        }

        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 2: GARBAGE COLLECT (Periodic)                           ║
        // ╚═══════════════════════════════════════════════════════════════╝

        if now.saturating_sub(self.last_gc) > self.gc_interval_secs {
            self.garbage_collect(now);
            self.last_gc = now;
        }

        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 3: NONCE CHECK                                          ║
        // ╚═══════════════════════════════════════════════════════════════╝

        if self.cache.contains_key(&nonce) {
            return Err(NonceError::NonceReused { nonce });
        }

        // ╔═══════════════════════════════════════════════════════════════╗
        // ║  STEP 4: ADD NONCE                                            ║
        // ╚═══════════════════════════════════════════════════════════════╝

        self.cache.insert(nonce, timestamp);

        Ok(())
    }

    /// Check if a nonce exists without adding it.
    #[must_use]
    pub fn contains(&self, nonce: &Uuid) -> bool {
        self.cache.contains_key(nonce)
    }

    /// Get the number of cached nonces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Remove expired nonces from the cache.
    fn garbage_collect(&mut self, now: u64) {
        let expiry_threshold = now.saturating_sub(self.validity_window_secs);
        self.cache.retain(|_, &mut ts| ts > expiry_threshold);
    }

    /// Get current Unix timestamp.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

impl Default for TimeBoundedNonceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        TimeBoundedNonceCache::current_timestamp()
    }

    #[test]
    fn test_valid_nonce() {
        let mut cache = TimeBoundedNonceCache::new();
        let nonce = Uuid::new_v4();
        let timestamp = now();

        assert!(cache.validate_and_add(nonce, timestamp).is_ok());
        assert!(cache.contains(&nonce));
    }

    #[test]
    fn test_duplicate_nonce_rejected() {
        let mut cache = TimeBoundedNonceCache::new();
        let nonce = Uuid::new_v4();
        let timestamp = now();

        assert!(cache.validate_and_add(nonce, timestamp).is_ok());

        let result = cache.validate_and_add(nonce, timestamp);
        assert!(matches!(result, Err(NonceError::NonceReused { .. })));
    }

    #[test]
    fn test_timestamp_too_old() {
        let mut cache = TimeBoundedNonceCache::new();
        let nonce = Uuid::new_v4();
        let old_timestamp = now().saturating_sub(120); // 2 minutes ago

        let result = cache.validate_and_add(nonce, old_timestamp);
        assert!(matches!(result, Err(NonceError::MessageTooOld { .. })));
    }

    #[test]
    fn test_timestamp_from_future() {
        let mut cache = TimeBoundedNonceCache::new();
        let nonce = Uuid::new_v4();
        let future_timestamp = now() + 60; // 1 minute in future

        let result = cache.validate_and_add(nonce, future_timestamp);
        assert!(matches!(result, Err(NonceError::MessageFromFuture { .. })));
    }

    #[test]
    fn test_timestamp_within_skew_allowed() {
        let mut cache = TimeBoundedNonceCache::new();

        // 5 seconds in future (within 10s skew)
        let nonce1 = Uuid::new_v4();
        let future_ok = now() + 5;
        assert!(cache.validate_and_add(nonce1, future_ok).is_ok());

        // 30 seconds in past (within 60s window)
        let nonce2 = Uuid::new_v4();
        let past_ok = now().saturating_sub(30);
        assert!(cache.validate_and_add(nonce2, past_ok).is_ok());
    }

    #[test]
    fn test_cache_length() {
        let mut cache = TimeBoundedNonceCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let timestamp = now();
        for _ in 0..5 {
            let nonce = Uuid::new_v4();
            cache.validate_and_add(nonce, timestamp).unwrap();
        }

        assert_eq!(cache.len(), 5);
        assert!(!cache.is_empty());
    }

    #[test]
    fn test_custom_config() {
        let cache = TimeBoundedNonceCache::with_config(60, 5);
        assert_eq!(cache.validity_window_secs, 60);
        assert_eq!(cache.gc_interval_secs, 5);
    }
}
