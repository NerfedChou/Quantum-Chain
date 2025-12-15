//! Tests for Security Adapters
//!
//! Reference: SPEC-01-PEER-DISCOVERY.md Security Section

use super::*;
use crate::ports::{RandomSource, RateLimiter, SecureHasher};

// =============================================================================
// RandomSource Tests
// =============================================================================

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

// =============================================================================
// SecureHasher Tests
// =============================================================================

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

// =============================================================================
// RateLimiter Tests
// =============================================================================

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

    let limiter =
        SlidingWindowRateLimiter::with_time_provider(move || time_clone.load(Ordering::SeqCst));

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
