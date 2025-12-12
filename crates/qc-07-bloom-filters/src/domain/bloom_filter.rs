//! Core Bloom Filter implementation
//!
//! Reference: SPEC-07 Section 2.1, System.md Subsystem 7
//!
//! INVARIANTS:
//! - INVARIANT-1: FPR = (1 - e^(-kn/m))^k <= target_fpr
//! - INVARIANT-2: No false negatives - if inserted, contains() MUST return true

use bitvec::prelude::*;
use serde::{Deserialize, Serialize};

use super::hash_functions::compute_hash_positions;
use super::parameters::{calculate_fpr, calculate_optimal_parameters};

/// Bloom filter for probabilistic membership testing
///
/// A Bloom filter is a space-efficient probabilistic data structure that
/// can test whether an element is a member of a set. False positives are
/// possible, but false negatives are not.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomFilter {
    /// Bit array storing the filter state
    #[serde(with = "bitvec_serde")]
    bits: BitVec<u8, Lsb0>,
    /// Number of hash functions (k)
    k: usize,
    /// Size in bits (m)
    m: usize,
    /// Number of elements inserted (n)
    n: usize,
    /// Tweak for hash function variation (privacy rotation)
    tweak: u32,
}

/// Serde support for BitVec
mod bitvec_serde {
    use bitvec::prelude::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bits: &BitVec<u8, Lsb0>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes: Vec<u8> = bits.as_raw_slice().to_vec();
        (bytes, bits.len()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BitVec<u8, Lsb0>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (bytes, len): (Vec<u8>, usize) = Deserialize::deserialize(deserializer)?;
        let mut bits = BitVec::<u8, Lsb0>::from_vec(bytes);
        bits.truncate(len);
        Ok(bits)
    }
}

impl BloomFilter {
    /// Create a new Bloom filter with specified parameters
    ///
    /// # Arguments
    /// * `m` - Size in bits
    /// * `k` - Number of hash functions
    pub fn new(m: usize, k: usize) -> Self {
        Self {
            bits: bitvec![u8, Lsb0; 0; m],
            k,
            m,
            n: 0,
            tweak: 0,
        }
    }

    /// Create a new Bloom filter with optimal parameters for target FPR
    ///
    /// # Arguments
    /// * `expected_elements` - Expected number of elements (n)
    /// * `target_fpr` - Target false positive rate
    pub fn new_with_fpr(expected_elements: usize, target_fpr: f64) -> Self {
        let params = calculate_optimal_parameters(expected_elements, target_fpr);
        Self::new(params.size_bits, params.hash_count)
    }

    /// Create a new Bloom filter with a specific tweak for privacy rotation
    pub fn new_with_tweak(m: usize, k: usize, tweak: u32) -> Self {
        Self {
            bits: bitvec![u8, Lsb0; 0; m],
            k,
            m,
            n: 0,
            tweak,
        }
    }

    /// Insert an element into the filter
    ///
    /// After insertion, `contains(element)` is guaranteed to return true.
    /// This is INVARIANT-2: No false negatives.
    pub fn insert(&mut self, element: &[u8]) {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);
        for pos in positions {
            self.bits.set(pos, true);
        }
        self.n += 1;
    }

    /// Test if an element might be in the filter
    ///
    /// Returns:
    /// - `true` if the element might be in the set (could be false positive)
    /// - `false` if the element is definitely NOT in the set (never false negative)
    ///
    /// INVARIANT-2: If an element was inserted, this MUST return true.
    pub fn contains(&self, element: &[u8]) -> bool {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);
        positions.iter().all(|&pos| self.bits[pos])
    }

    /// Test if an element might be in the filter (constant-time)
    ///
    /// **SECURITY**: This method provides side-channel resistance by:
    /// 1. Always checking ALL hash positions (no early exit)
    /// 2. Using bitwise AND accumulator (branchless)
    /// 3. Ensuring constant CPU cycles regardless of match
    ///
    /// Use this method when the filter is being queried by external parties
    /// to prevent timing attacks that could reveal filter contents.
    ///
    /// Reference: SPEC-07 Appendix B.3 - Privacy Considerations
    pub fn contains_constant_time(&self, element: &[u8]) -> bool {
        let positions = compute_hash_positions(element, self.k, self.m, self.tweak);

        // Branchless accumulator - always accesses all positions
        let mut result: u8 = 1;
        for &pos in &positions {
            // Always read the bit, always AND with accumulator
            let bit = self.bits[pos] as u8;
            result &= bit;
        }

        result == 1
    }

    /// Merge another filter into this one (OR operation)
    ///
    /// After merge, this filter will match all elements from both filters.
    /// Filters must have the same parameters (m, k, tweak).
    ///
    /// # Performance
    /// Uses optimized bitwise OR on underlying byte slices - O(m/8) operations
    /// instead of O(m) bit-by-bit operations.
    ///
    /// # Panics
    /// Panics if filters have different m or k values.
    pub fn merge(&mut self, other: &BloomFilter) {
        assert_eq!(self.m, other.m, "Cannot merge filters with different sizes");
        assert_eq!(
            self.k, other.k,
            "Cannot merge filters with different hash counts"
        );

        // Optimized: OR the underlying byte slices directly
        let self_raw = self.bits.as_raw_mut_slice();
        let other_raw = other.bits.as_raw_slice();
        for (s, o) in self_raw.iter_mut().zip(other_raw.iter()) {
            *s |= *o;
        }
        self.n += other.n;
    }

    /// Calculate the current false positive rate
    ///
    /// Formula: FPR = (1 - e^(-kn/m))^k
    pub fn false_positive_rate(&self) -> f64 {
        calculate_fpr(self.m, self.n, self.k)
    }

    /// Get the number of bits set in the filter
    pub fn bits_set(&self) -> usize {
        self.bits.count_ones()
    }

    /// Get the filter size in bits
    pub fn size_bits(&self) -> usize {
        self.m
    }

    /// Get the number of hash functions
    pub fn hash_count(&self) -> usize {
        self.k
    }

    /// Get the number of elements inserted
    pub fn elements_inserted(&self) -> usize {
        self.n
    }

    /// Get the tweak value
    pub fn tweak(&self) -> u32 {
        self.tweak
    }

    /// Set a new tweak (for filter rotation)
    pub fn set_tweak(&mut self, tweak: u32) {
        self.tweak = tweak;
    }

    /// Clear the filter (reset all bits to 0)
    pub fn clear(&mut self) {
        self.bits.fill(false);
        self.n = 0;
    }

    /// Serialize the filter to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Deserialize a filter from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes).map_err(|e| e.to_string())
    }

    /// Calculate optimal parameters for given constraints
    pub fn optimal_params(n: usize, fpr: f64) -> (usize, usize) {
        let params = calculate_optimal_parameters(n, fpr);
        (params.hash_count, params.size_bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_new_creates_valid_filter() {
        let filter = BloomFilter::new(1000, 7);

        assert_eq!(filter.m, 1000, "Filter size should be 1000 bits");
        assert_eq!(filter.k, 7, "Filter should have 7 hash functions");
        assert_eq!(filter.n, 0, "Filter should have 0 elements initially");
        assert_eq!(filter.bits_set(), 0, "All bits should be zero initially");
    }

    #[test]
    fn test_bloom_filter_insert_sets_bits() {
        let mut filter = BloomFilter::new(1000, 7);
        let element = b"test_element_0xABCD1234";

        assert_eq!(filter.bits_set(), 0, "Initially no bits set");

        filter.insert(element);

        assert!(
            filter.bits_set() > 0,
            "After insert, some bits should be set"
        );
        assert!(
            filter.bits_set() <= 7,
            "At most k=7 bits should be set for one element"
        );

        // Insert same element again - bits should stay the same (deterministic)
        let bits_before = filter.bits_set();
        filter.insert(element);
        // Note: n increases but unique bits don't change for same element
        assert!(
            filter.bits_set() >= bits_before,
            "Bits set should not decrease"
        );
    }

    #[test]
    fn test_bloom_filter_contains_after_insert() {
        let mut filter = BloomFilter::new(1000, 7);
        let element = b"0xABCD1234567890ABCDEF";

        filter.insert(element);

        assert!(
            filter.contains(element),
            "INVARIANT-2: contains() must return true for inserted element"
        );
    }

    #[test]
    fn test_bloom_filter_no_false_negatives_bulk() {
        let mut filter = BloomFilter::new(10000, 7);
        let elements: Vec<String> = (0..1000).map(|i| format!("address_{:04x}", i)).collect();

        // Insert all elements
        for elem in &elements {
            filter.insert(elem.as_bytes());
        }

        // INVARIANT-2: ALL inserted elements MUST be found
        for elem in &elements {
            assert!(
                filter.contains(elem.as_bytes()),
                "INVARIANT-2 VIOLATED: False negative for {}",
                elem
            );
        }
    }

    #[test]
    fn test_bloom_filter_false_positive_rate_bounded() {
        let target_fpr = 0.01; // 1%
        let n = 100;
        let mut filter = BloomFilter::new_with_fpr(n, target_fpr);

        // Insert n elements
        for i in 0..n {
            filter.insert(format!("inserted_{}", i).as_bytes());
        }

        // Test 100,000 elements that were NOT inserted
        let mut false_positives = 0;
        for i in 0..100_000 {
            if filter.contains(format!("not_inserted_{}", i).as_bytes()) {
                false_positives += 1;
            }
        }

        let actual_fpr = false_positives as f64 / 100_000.0;

        // INVARIANT-1: FPR should be bounded
        // Allow 1.5x statistical tolerance
        assert!(
            actual_fpr <= target_fpr * 1.5,
            "INVARIANT-1: Actual FPR {} exceeds 1.5 * target {}",
            actual_fpr,
            target_fpr
        );
    }

    #[test]
    fn test_bloom_filter_merge() {
        let mut filter1 = BloomFilter::new(1000, 7);
        let mut filter2 = BloomFilter::new(1000, 7);

        let addr_a = b"address_A";
        let addr_b = b"address_B";
        let addr_c = b"address_C";
        let addr_d = b"address_D";

        filter1.insert(addr_a);
        filter1.insert(addr_b);
        filter2.insert(addr_c);
        filter2.insert(addr_d);

        // Merge filter2 into filter1
        filter1.merge(&filter2);

        // Merged filter should contain all elements
        assert!(filter1.contains(addr_a), "Merged filter should contain A");
        assert!(filter1.contains(addr_b), "Merged filter should contain B");
        assert!(filter1.contains(addr_c), "Merged filter should contain C");
        assert!(filter1.contains(addr_d), "Merged filter should contain D");
    }

    #[test]
    fn test_bloom_filter_serialization() {
        let mut filter = BloomFilter::new(1000, 7);
        filter.insert(b"element_1");
        filter.insert(b"element_2");
        filter.insert(b"element_3");

        // Serialize
        let bytes = filter.to_bytes();
        assert!(!bytes.is_empty(), "Serialization should produce bytes");

        // Deserialize
        let restored = BloomFilter::from_bytes(&bytes).expect("Deserialization should succeed");

        // Verify all contains() results are identical
        assert!(restored.contains(b"element_1"));
        assert!(restored.contains(b"element_2"));
        assert!(restored.contains(b"element_3"));
        assert_eq!(restored.m, filter.m);
        assert_eq!(restored.k, filter.k);
        assert_eq!(restored.n, filter.n);
    }

    #[test]
    fn test_optimal_parameters_calculation() {
        // For n=50, FPR=0.0001 → expect k≈13, m≈959
        let (k, m) = BloomFilter::optimal_params(50, 0.0001);
        assert!(k >= 10 && k <= 15, "Expected k≈13, got k={}", k);
        assert!(m >= 800 && m <= 1200, "Expected m≈959, got m={}", m);

        // For n=100, FPR=0.01 → expect k≈7, m≈959
        let (k, m) = BloomFilter::optimal_params(100, 0.01);
        assert!(k >= 5 && k <= 9, "Expected k≈7, got k={}", k);
        assert!(m >= 800 && m <= 1200, "Expected m≈959, got m={}", m);
    }

    #[test]
    fn test_filter_rotation_changes_bits() {
        let addresses = [b"wallet_0x1234".as_slice(), b"wallet_0x5678".as_slice()];

        let mut filter1 = BloomFilter::new_with_tweak(1000, 7, 0);
        let mut filter2 = BloomFilter::new_with_tweak(1000, 7, 12345);

        for addr in &addresses {
            filter1.insert(addr);
            filter2.insert(addr);
        }

        // Same addresses with different tweak → different bit positions
        // (comparing bit arrays directly)
        let bits1: Vec<bool> = (0..1000).map(|i| filter1.bits[i]).collect();
        let bits2: Vec<bool> = (0..1000).map(|i| filter2.bits[i]).collect();

        assert_ne!(
            bits1, bits2,
            "Different tweaks should produce different bit patterns for privacy rotation"
        );
    }
}
