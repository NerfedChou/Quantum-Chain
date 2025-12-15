//! Random Source Adapters

use crate::ports::RandomSource;
use std::sync::Mutex;

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
