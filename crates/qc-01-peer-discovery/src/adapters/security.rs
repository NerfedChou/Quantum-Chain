//! # Security Adapters (V2.5)
//!
//! Implements the security port traits for hardening:
//! - `RandomSource`: Cryptographically secure random number generation
//! - `SecureHasher`: DoS-resistant keyed hashing
//! - `RateLimiter`: Domain-level rate limiting backstop
//!
//! ## Mock vs Production
//!
//! | Adapter | Mock (Testing) | Production |
//! |---------|----------------|------------|
//! | `RandomSource` | `FixedRandomSource` | `OsRandomSource` |
//! | `SecureHasher` | `SimpleHasher` | `SipHasher` |
//! | `RateLimiter` | `NoOpRateLimiter` | `SlidingWindowRateLimiter` |

use crate::ports::{RandomSource, RateLimiter, SecureHasher};
use std::collections::HashMap;
use std::sync::Mutex;

// =============================================================================
// RANDOM SOURCE ADAPTERS
// =============================================================================

/// Fixed random source for deterministic testing.
///
/// Always returns the same sequence of values, enabling reproducible tests.
///
/// # Example
///
/// ```rust
/// use qc_01_peer_discovery::adapters::security::FixedRandomSource;
/// use qc_01_peer_discovery::RandomSource;
///
/// let rng = FixedRandomSource::new(42);
/// assert_eq!(rng.random_usize(100), 42);
/// assert_eq!(rng.random_usize(100), 42); // Always same value
/// ```
#[derive(Debug, Clone)]
pub struct FixedRandomSource {
    value: usize,
}

impl FixedRandomSource {
    /// Create a fixed random source that always returns the given value.
    pub fn new(value: usize) -> Self {
        Self { value }
    }

    /// Create a random source that returns 0 (first element).
    pub fn first() -> Self {
        Self::new(0)
    }
}

impl RandomSource for FixedRandomSource {
    fn random_usize(&self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            self.value % max
        }
    }

    fn shuffle_slice(&self, _slice: &mut [u8]) {
        // No-op: deterministic tests skip shuffling
    }
}

/// Production random source using OS entropy.
///
/// Uses a simple LCG seeded from time for now.
/// For true CSPRNG, add `rand` crate dependency.
///
/// # Security Note
///
/// This is a placeholder implementation. For production use,
/// replace with `rand::rngs::OsRng` or similar CSPRNG.
#[derive(Debug)]
pub struct OsRandomSource {
    /// Simple state for demonstration (NOT cryptographically secure)
    state: Mutex<u64>,
}

impl OsRandomSource {
    /// Create a new OS random source.
    ///
    /// Seeds from current time (placeholder for actual OS RNG).
    pub fn new() -> Self {
        // Seed from system time nanoseconds
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);

        Self {
            state: Mutex::new(seed),
        }
    }
}

impl Default for OsRandomSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomSource for OsRandomSource {
    fn random_usize(&self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }

        let mut state = self.state.lock().unwrap();
        // LCG: x_{n+1} = (a * x_n + c) mod m
        // Using glibc constants
        *state = state.wrapping_mul(1103515245).wrapping_add(12345);
        ((*state >> 16) as usize) % max
    }

    fn shuffle_slice(&self, slice: &mut [u8]) {
        let len = slice.len();
        if len < 2 {
            return;
        }

        // Fisher-Yates shuffle
        for i in (1..len).rev() {
            let j = self.random_usize(i + 1);
            slice.swap(i, j);
        }
    }
}

// =============================================================================
// SECURE HASHER ADAPTERS
// =============================================================================

/// Simple hasher for testing (NOT DoS-resistant).
///
/// Uses a basic multiplicative hash for deterministic test behavior.
#[derive(Debug, Clone)]
pub struct SimpleHasher {
    key: u64,
}

impl SimpleHasher {
    /// Create a simple hasher with given key.
    pub fn new(key: u64) -> Self {
        Self { key }
    }

    /// Create with default key (0).
    pub fn default_key() -> Self {
        Self::new(0)
    }
}

impl Default for SimpleHasher {
    fn default() -> Self {
        Self::default_key()
    }
}

impl SecureHasher for SimpleHasher {
    fn hash(&self, data: &[u8]) -> u64 {
        let mut hash = self.key;
        for &byte in data {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }

    fn hash_combined(&self, a: &[u8], b: &[u8]) -> u64 {
        let mut hash = self.key;
        for &byte in a {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        for &byte in b {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }
}

/// SipHash-based secure hasher (DoS-resistant).
///
/// Uses SipHash-2-4 which is designed to be:
/// - Fast for short inputs
/// - Resistant to hash-flooding DoS attacks
///
/// # Key Management
///
/// The key should be:
/// - Generated randomly on node startup
/// - NOT derived from predictable values
/// - Kept secret (not transmitted)
#[derive(Debug, Clone)]
pub struct SipHasher {
    key: [u8; 16],
}

impl SipHasher {
    /// Create a SipHasher with the given 128-bit key.
    pub fn new(key: [u8; 16]) -> Self {
        Self { key }
    }

    /// Create with random key (using OsRandomSource).
    pub fn with_random_key() -> Self {
        let rng = OsRandomSource::new();
        let mut key = [0u8; 16];
        rng.shuffle_slice(&mut key);
        // Fill with random-ish values
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = rng.random_usize(256) as u8 ^ (i as u8);
        }
        Self { key }
    }

    /// SipHash-2-4 implementation.
    fn siphash(&self, data: &[u8]) -> u64 {
        // Extract key halves
        let k0 = u64::from_le_bytes([
            self.key[0],
            self.key[1],
            self.key[2],
            self.key[3],
            self.key[4],
            self.key[5],
            self.key[6],
            self.key[7],
        ]);
        let k1 = u64::from_le_bytes([
            self.key[8],
            self.key[9],
            self.key[10],
            self.key[11],
            self.key[12],
            self.key[13],
            self.key[14],
            self.key[15],
        ]);

        // SipHash initialization
        let mut v0 = k0 ^ 0x736f6d6570736575;
        let mut v1 = k1 ^ 0x646f72616e646f6d;
        let mut v2 = k0 ^ 0x6c7967656e657261;
        let mut v3 = k1 ^ 0x7465646279746573;

        let len = data.len();
        let blocks = len / 8;

        // Process 8-byte blocks
        for i in 0..blocks {
            let offset = i * 8;
            let m = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            v3 ^= m;
            Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
            Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
            v0 ^= m;
        }

        // Process remaining bytes
        let mut last = (len as u64) << 56;
        let remaining = &data[blocks * 8..];
        for (i, &byte) in remaining.iter().enumerate() {
            last |= (byte as u64) << (i * 8);
        }

        v3 ^= last;
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        v0 ^= last;

        // Finalization
        v2 ^= 0xff;
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);

        v0 ^ v1 ^ v2 ^ v3
    }

    #[inline]
    fn sipround(v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64) {
        *v0 = v0.wrapping_add(*v1);
        *v1 = v1.rotate_left(13);
        *v1 ^= *v0;
        *v0 = v0.rotate_left(32);
        *v2 = v2.wrapping_add(*v3);
        *v3 = v3.rotate_left(16);
        *v3 ^= *v2;
        *v0 = v0.wrapping_add(*v3);
        *v3 = v3.rotate_left(21);
        *v3 ^= *v0;
        *v2 = v2.wrapping_add(*v1);
        *v1 = v1.rotate_left(17);
        *v1 ^= *v2;
        *v2 = v2.rotate_left(32);
    }
}

impl SecureHasher for SipHasher {
    fn hash(&self, data: &[u8]) -> u64 {
        self.siphash(data)
    }

    fn hash_combined(&self, a: &[u8], b: &[u8]) -> u64 {
        // Concatenate and hash
        let mut combined = Vec::with_capacity(a.len() + b.len());
        combined.extend_from_slice(a);
        combined.extend_from_slice(b);
        self.siphash(&combined)
    }
}

// =============================================================================
// RATE LIMITER ADAPTERS
// =============================================================================

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

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // RandomSource Tests
    // =========================================================================

    #[test]
    fn test_fixed_random_source_deterministic() {
        let rng = FixedRandomSource::new(42);
        assert_eq!(rng.random_usize(100), 42);
        assert_eq!(rng.random_usize(100), 42);
        assert_eq!(rng.random_usize(100), 42);
    }

    #[test]
    fn test_fixed_random_source_modulo() {
        let rng = FixedRandomSource::new(150);
        assert_eq!(rng.random_usize(100), 50); // 150 % 100
    }

    #[test]
    fn test_os_random_source_in_range() {
        let rng = OsRandomSource::new();
        for _ in 0..100 {
            let val = rng.random_usize(10);
            assert!(val < 10);
        }
    }

    #[test]
    fn test_os_random_source_shuffle() {
        let rng = OsRandomSource::new();
        let mut original = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let copy = original;
        rng.shuffle_slice(&mut original);
        // Shuffled should be different (very high probability)
        // Note: There's a tiny chance they're the same, but it's 1/40320
        assert_ne!(original, copy);
    }

    // =========================================================================
    // SecureHasher Tests
    // =========================================================================

    #[test]
    fn test_simple_hasher_deterministic() {
        let hasher = SimpleHasher::new(0);
        let h1 = hasher.hash(b"hello");
        let h2 = hasher.hash(b"hello");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_simple_hasher_different_inputs() {
        let hasher = SimpleHasher::new(0);
        let h1 = hasher.hash(b"hello");
        let h2 = hasher.hash(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_sip_hasher_deterministic() {
        let hasher = SipHasher::new([0u8; 16]);
        let h1 = hasher.hash(b"test data");
        let h2 = hasher.hash(b"test data");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_sip_hasher_key_matters() {
        let h1 = SipHasher::new([0u8; 16]).hash(b"test");
        let h2 = SipHasher::new([1u8; 16]).hash(b"test");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_sip_hasher_combined() {
        let hasher = SipHasher::new([0u8; 16]);
        let h1 = hasher.hash_combined(b"hello", b"world");
        let h2 = hasher.hash(b"helloworld");
        assert_eq!(h1, h2);
    }

    // =========================================================================
    // RateLimiter Tests
    // =========================================================================

    #[test]
    fn test_noop_rate_limiter_always_allows() {
        let limiter = NoOpRateLimiter::new();
        for _ in 0..1000 {
            assert!(limiter.check_rate(b"key", 1, 1));
        }
    }

    #[test]
    fn test_sliding_window_allows_under_limit() {
        let time = std::sync::atomic::AtomicU64::new(1000);
        let limiter = SlidingWindowRateLimiter::with_time_provider(move || {
            time.load(std::sync::atomic::Ordering::SeqCst)
        });

        // Should allow 5 requests
        for _ in 0..5 {
            assert!(limiter.check_rate(b"key", 5, 60));
        }

        // 6th should be blocked
        assert!(!limiter.check_rate(b"key", 5, 60));
    }

    #[test]
    fn test_sliding_window_resets_after_window() {
        use std::sync::atomic::{AtomicU64, Ordering};
        let time = std::sync::Arc::new(AtomicU64::new(1000));
        let time_clone = time.clone();

        let limiter = SlidingWindowRateLimiter::with_time_provider(move || {
            time_clone.load(Ordering::SeqCst)
        });

        // Use up all requests
        assert!(limiter.check_rate(b"key", 2, 60));
        assert!(limiter.check_rate(b"key", 2, 60));
        assert!(!limiter.check_rate(b"key", 2, 60));

        // Advance time past window
        time.store(1065, Ordering::SeqCst);

        // Should allow again
        assert!(limiter.check_rate(b"key", 2, 60));
    }

    #[test]
    fn test_sliding_window_different_keys() {
        let limiter = SlidingWindowRateLimiter::new();

        // Each key has its own limit
        assert!(limiter.check_rate(b"key1", 1, 60));
        assert!(!limiter.check_rate(b"key1", 1, 60));

        assert!(limiter.check_rate(b"key2", 1, 60));
        assert!(!limiter.check_rate(b"key2", 1, 60));
    }
}
