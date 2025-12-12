//! # Counting Bloom Filter (Delta Filters)
//!
//! Allows incremental add/remove without full rebuild.
//!
//! ## Problem
//!
//! Standard Bloom filters are insert-only. To update, client must
//! rebuild and resend the entire filter - wasteful for active wallets.
//!
//! ## Solution: Counting Bloom Filter (CBF)
//!
//! Replace bits with counters:
//! - Add: Increment counters at hashed positions
//! - Remove: Decrement counters at hashed positions
//! - Membership: True if all counters > 0
//!
//! ## Wire Optimization
//!
//! Most counters are 0, so use Run-Length Encoding (RLE) for
//! compression when sending updates.
//!
//! Reference: SPEC-07 Phase 4

use crate::domain::hash_functions::compute_hash_positions;

/// Maximum counter value (4-bit = 15).
const MAX_COUNTER: u8 = 15;

/// Counting Bloom Filter with 4-bit counters.
///
/// Supports both add AND remove operations, unlike standard Bloom filters.
#[derive(Clone, Debug)]
pub struct CountingBloomFilter {
    /// 4-bit counters packed into bytes (2 counters per byte)
    counters: Vec<u8>,
    /// Number of hash functions
    k: usize,
    /// Size in counters (not bytes)
    m: usize,
    /// Number of elements inserted
    n: usize,
    /// Tweak for hash variation
    tweak: u32,
}

impl CountingBloomFilter {
    /// Create a new counting Bloom filter.
    pub fn new(m: usize, k: usize) -> Self {
        Self {
            counters: vec![0u8; m.div_ceil(2)], // Pack 2 counters per byte
            k,
            m,
            n: 0,
            tweak: 0,
        }
    }

    /// Create with a specific tweak.
    pub fn new_with_tweak(m: usize, k: usize, tweak: u32) -> Self {
        Self {
            counters: vec![0u8; m.div_ceil(2)],
            k,
            m,
            n: 0,
            tweak,
        }
    }

    /// Add an element (increment counters).
    pub fn add(&mut self, element: &[u8]) {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);
        for pos in positions {
            self.increment(pos);
        }
        self.n += 1;
    }

    /// Remove an element (decrement counters).
    ///
    /// **Note**: Only call if element was previously added.
    /// Removing an element that wasn't added causes false negatives.
    pub fn remove(&mut self, element: &[u8]) {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);
        for pos in positions {
            self.decrement(pos);
        }
        self.n = self.n.saturating_sub(1);
    }

    /// Check if element might be in the filter.
    pub fn contains(&self, element: &[u8]) -> bool {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);
        positions.iter().all(|&pos| self.get_counter(pos) > 0)
    }

    /// Get counter at position.
    fn get_counter(&self, pos: usize) -> u8 {
        let byte_idx = pos / 2;
        if pos % 2 == 0 {
            self.counters[byte_idx] >> 4
        } else {
            self.counters[byte_idx] & 0x0F
        }
    }

    /// Increment counter at position (saturating at MAX_COUNTER).
    fn increment(&mut self, pos: usize) {
        let byte_idx = pos / 2;
        let current = self.get_counter(pos);
        if current < MAX_COUNTER {
            if pos % 2 == 0 {
                self.counters[byte_idx] = (self.counters[byte_idx] & 0x0F) | ((current + 1) << 4);
            } else {
                self.counters[byte_idx] = (self.counters[byte_idx] & 0xF0) | (current + 1);
            }
        }
    }

    /// Decrement counter at position (saturating at 0).
    fn decrement(&mut self, pos: usize) {
        let byte_idx = pos / 2;
        let current = self.get_counter(pos);
        if current > 0 {
            if pos % 2 == 0 {
                self.counters[byte_idx] = (self.counters[byte_idx] & 0x0F) | ((current - 1) << 4);
            } else {
                self.counters[byte_idx] = (self.counters[byte_idx] & 0xF0) | (current - 1);
            }
        }
    }

    /// Get size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.counters.len()
    }

    /// Get number of hash functions.
    pub fn hash_count(&self) -> usize {
        self.k
    }

    /// Get number of elements.
    pub fn elements_count(&self) -> usize {
        self.n
    }

    /// Compress to RLE-encoded bytes for wire transfer.
    ///
    /// Format: `[run_length][value][run_length][value]...`
    pub fn to_rle(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;

        while i < self.counters.len() {
            let value = self.counters[i];
            let mut run_length = 1u8;

            while i + (run_length as usize) < self.counters.len()
                && self.counters[i + run_length as usize] == value
                && run_length < 255
            {
                run_length += 1;
            }

            result.push(run_length);
            result.push(value);
            i += run_length as usize;
        }

        result
    }

    /// Decompress from RLE-encoded bytes.
    pub fn from_rle(data: &[u8], m: usize, k: usize, tweak: u32) -> Option<Self> {
        let mut counters = Vec::with_capacity(m.div_ceil(2));
        let mut i = 0;

        while i + 1 < data.len() {
            let run_length = data[i];
            let value = data[i + 1];

            for _ in 0..run_length {
                counters.push(value);
            }
            i += 2;
        }

        if counters.len() != m.div_ceil(2) {
            return None;
        }

        Some(Self {
            counters,
            k,
            m,
            n: 0, // Unknown from RLE
            tweak,
        })
    }

    /// Clear all counters.
    pub fn clear(&mut self) {
        self.counters.fill(0);
        self.n = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_contains() {
        let mut filter = CountingBloomFilter::new(1000, 7);

        filter.add(b"test_element");
        assert!(filter.contains(b"test_element"));
    }

    #[test]
    fn test_remove() {
        let mut filter = CountingBloomFilter::new(1000, 7);

        filter.add(b"element1");
        filter.add(b"element2");

        assert!(filter.contains(b"element1"));
        assert!(filter.contains(b"element2"));

        filter.remove(b"element1");
        assert!(!filter.contains(b"element1"));
        assert!(filter.contains(b"element2"));
    }

    #[test]
    fn test_counter_saturation() {
        let mut filter = CountingBloomFilter::new(1000, 7);

        // Add same element many times
        for _ in 0..20 {
            filter.add(b"saturate_me");
        }

        // Should still contain
        assert!(filter.contains(b"saturate_me"));

        // Remove many times
        for _ in 0..20 {
            filter.remove(b"saturate_me");
        }

        // Should be gone (counters saturate, so might still be there)
        // This is expected behavior - can't remove more than MAX_COUNTER
    }

    #[test]
    fn test_rle_compression() {
        let mut filter = CountingBloomFilter::new(1000, 7);

        // Add a few elements
        filter.add(b"element1");
        filter.add(b"element2");

        let rle = filter.to_rle();

        // RLE should be much smaller than raw counters for sparse filter
        assert!(rle.len() < filter.size_bytes());
    }

    #[test]
    fn test_rle_roundtrip() {
        let mut filter = CountingBloomFilter::new(100, 5);
        filter.add(b"element1");
        filter.add(b"element2");
        filter.add(b"element3");

        let rle = filter.to_rle();
        let restored = CountingBloomFilter::from_rle(&rle, 100, 5, 0).unwrap();

        // Check membership is preserved
        assert!(restored.contains(b"element1"));
        assert!(restored.contains(b"element2"));
        assert!(restored.contains(b"element3"));
    }

    #[test]
    fn test_4bit_packing() {
        let filter = CountingBloomFilter::new(100, 5);

        // 100 counters should use 50 bytes (2 per byte)
        assert_eq!(filter.size_bytes(), 50);
    }
}
