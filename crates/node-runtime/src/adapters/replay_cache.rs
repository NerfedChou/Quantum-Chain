//! # Nonce Replay Cache
//!
//! Cuckoo filter-based anti-replay cache for IPC nonce deduplication.
//!
//! ## Security Properties
//!
//! - Prevents replay attacks by tracking seen nonces (UUIDs)
//! - Probabilistic with <5% false positive rate
//! - Supports deletion for capacity management
//! - Time-windowed to limit memory growth
//!
//! ## IPC Integration
//!
//! Check every incoming `AuthenticatedMessage.nonce` against this cache:
//! - If seen: reject message
//! - If fresh: insert and process

#[cfg(feature = "qc-07")]
use qc_07_bloom_filters::CuckooFilter; // Layer compliant

use std::time::{Duration, Instant};
use uuid::Uuid;

/// Default cache capacity (100k nonces).
const DEFAULT_CAPACITY: usize = 100_000;

/// Default window duration (10 minutes).
const DEFAULT_WINDOW_SECS: u64 = 600;

/// Anti-replay cache for IPC message nonces.
///
/// Uses a Cuckoo filter for memory-efficient probabilistic tracking
/// with support for deletion when rotating windows.
#[cfg(feature = "qc-07")]
pub struct NonceReplayCache {
    /// Cuckoo filter for current window
    current: CuckooFilter,
    /// Previous window (for overlap handling)
    previous: CuckooFilter,
    /// Capacity per window
    capacity: usize,
    /// Window start time
    window_start: Instant,
    /// Window duration
    window_duration: Duration,
    /// Total nonces checked
    total_checked: u64,
    /// Total replays detected
    replays_detected: u64,
}

#[cfg(feature = "qc-07")]
impl NonceReplayCache {
    /// Create new cache with default settings.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY, Duration::from_secs(DEFAULT_WINDOW_SECS))
    }

    /// Create cache with custom capacity and window duration.
    pub fn with_capacity(capacity: usize, window_duration: Duration) -> Self {
        Self {
            current: CuckooFilter::new(capacity),
            previous: CuckooFilter::new(capacity),
            capacity,
            window_start: Instant::now(),
            window_duration,
            total_checked: 0,
            replays_detected: 0,
        }
    }

    /// Check if nonce is a replay (seen before).
    ///
    /// Returns `true` if this is a REPLAY (should reject).
    /// Returns `false` if nonce is fresh (should accept).
    pub fn is_replay(&mut self, nonce: &Uuid) -> bool {
        self.total_checked += 1;
        self.maybe_rotate();

        let nonce_bytes = nonce.as_bytes();

        // Check both windows for potential replay
        if self.current.contains(nonce_bytes) || self.previous.contains(nonce_bytes) {
            self.replays_detected += 1;
            return true;
        }

        // Fresh nonce - insert into current window
        self.current.insert(nonce_bytes);
        false
    }

    /// Check without inserting (read-only query).
    pub fn was_seen(&self, nonce: &Uuid) -> bool {
        let nonce_bytes = nonce.as_bytes();
        self.current.contains(nonce_bytes) || self.previous.contains(nonce_bytes)
    }

    /// Mark a nonce as seen (for async verification workflows).
    pub fn mark_seen(&mut self, nonce: &Uuid) {
        self.maybe_rotate();
        self.current.insert(nonce.as_bytes());
    }

    /// Rotate windows if expired.
    fn maybe_rotate(&mut self) {
        if self.window_start.elapsed() > self.window_duration {
            // Swap: current becomes previous, create fresh current
            self.previous = std::mem::replace(&mut self.current, CuckooFilter::new(self.capacity));
            self.window_start = Instant::now();
        }
    }

    /// Get statistics.
    pub fn stats(&self) -> ReplayCacheStats {
        ReplayCacheStats {
            total_checked: self.total_checked,
            replays_detected: self.replays_detected,
            current_entries: self.current.len(),
            previous_entries: self.previous.len(),
            load_factor: self.current.load_factor(),
        }
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.current = CuckooFilter::new(self.capacity);
        self.previous = CuckooFilter::new(self.capacity);
        self.window_start = Instant::now();
    }
}

#[cfg(feature = "qc-07")]
impl Default for NonceReplayCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics.
#[derive(Debug, Clone)]
pub struct ReplayCacheStats {
    /// Total nonces checked
    pub total_checked: u64,
    /// Replays detected
    pub replays_detected: u64,
    /// Entries in current window
    pub current_entries: usize,
    /// Entries in previous window
    pub previous_entries: usize,
    /// Current window load factor
    pub load_factor: f64,
}

#[cfg(all(test, feature = "qc-07"))]
mod tests {
    use super::*;

    #[test]
    fn test_fresh_nonce_allowed() {
        let mut cache = NonceReplayCache::new();
        let nonce = Uuid::new_v4();

        assert!(!cache.is_replay(&nonce)); // Fresh
    }

    #[test]
    fn test_replay_detected() {
        let mut cache = NonceReplayCache::new();
        let nonce = Uuid::new_v4();

        assert!(!cache.is_replay(&nonce)); // First time - fresh
        assert!(cache.is_replay(&nonce)); // Second time - replay!
    }

    #[test]
    fn test_different_nonces_allowed() {
        let mut cache = NonceReplayCache::new();

        for _ in 0..100 {
            let nonce = Uuid::new_v4();
            assert!(!cache.is_replay(&nonce));
        }

        assert_eq!(cache.stats().replays_detected, 0);
    }

    #[test]
    fn test_stats() {
        let mut cache = NonceReplayCache::new();
        let nonce = Uuid::new_v4();

        cache.is_replay(&nonce);
        cache.is_replay(&nonce);

        let stats = cache.stats();
        assert_eq!(stats.total_checked, 2);
        assert_eq!(stats.replays_detected, 1);
    }
}
