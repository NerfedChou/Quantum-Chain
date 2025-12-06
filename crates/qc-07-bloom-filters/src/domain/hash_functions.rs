//! Hash functions for Bloom filter
//!
//! Reference: System.md, Subsystem 7 - Multiple hash functions (k=3-7)
//!
//! Uses MurmurHash3 for fast, high-quality hashing with different seeds.

use std::io::Cursor;

/// Hash an element with MurmurHash3 using a seed and tweak
///
/// The combination of seed (for different hash functions) and tweak
/// (for filter rotation) ensures independent hash outputs.
pub fn murmur_hash(element: &[u8], seed: u32, tweak: u32) -> u64 {
    let combined_seed = seed.wrapping_add(tweak);
    let mut cursor = Cursor::new(element);

    // Use murmur3 128-bit hash and take the lower 64 bits
    let hash = murmur3::murmur3_x64_128(&mut cursor, combined_seed).unwrap_or(0);
    hash as u64
}

/// Compute k hash positions for an element
///
/// Uses double hashing technique: h(i) = h1 + i * h2
/// This is more efficient than computing k independent hashes.
pub fn compute_hash_positions(element: &[u8], k: usize, m: usize, tweak: u32) -> Vec<usize> {
    let h1 = murmur_hash(element, 0, tweak);
    let h2 = murmur_hash(element, 1, tweak);

    (0..k)
        .map(|i| {
            let hash = h1.wrapping_add((i as u64).wrapping_mul(h2));
            (hash % m as u64) as usize
        })
        .collect()
}

/// Verify that hash function is deterministic
pub fn hash_is_deterministic(element: &[u8], seed: u32, tweak: u32) -> bool {
    let h1 = murmur_hash(element, seed, tweak);
    let h2 = murmur_hash(element, seed, tweak);
    h1 == h2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_murmur3_hash_deterministic() {
        let element = b"test_address_0xABCD";
        let seed = 42;
        let tweak = 100;

        let hash1 = murmur_hash(element, seed, tweak);
        let hash2 = murmur_hash(element, seed, tweak);

        assert_eq!(
            hash1, hash2,
            "Same input with same seed/tweak must produce same output"
        );
    }

    #[test]
    fn test_murmur3_different_seed_different_output() {
        let element = b"test_address_0xABCD";
        let tweak = 100;

        let hash1 = murmur_hash(element, 0, tweak);
        let hash2 = murmur_hash(element, 1, tweak);

        assert_ne!(
            hash1, hash2,
            "Different seeds must produce different outputs"
        );
    }

    #[test]
    fn test_murmur3_different_tweak_different_output() {
        let element = b"test_address_0xABCD";
        let seed = 42;

        let hash1 = murmur_hash(element, seed, 0);
        let hash2 = murmur_hash(element, seed, 100);

        assert_ne!(
            hash1, hash2,
            "Different tweaks must produce different outputs"
        );
    }

    #[test]
    fn test_multiple_hash_functions_independent() {
        let element = b"test_address_0xABCD";
        let k = 7;
        let m = 10000;
        let tweak = 0;

        let positions = compute_hash_positions(element, k, m, tweak);

        assert_eq!(positions.len(), k, "Should produce k positions");

        // All positions should be within bounds
        for pos in &positions {
            assert!(*pos < m, "Position {} should be < m={}", pos, m);
        }

        // At least some positions should be different (with high probability for k=7)
        let unique: std::collections::HashSet<_> = positions.iter().collect();
        assert!(
            unique.len() >= 3,
            "Hash functions should produce varied positions"
        );
    }

    #[test]
    fn test_tweak_affects_hash_output() {
        let element = b"wallet_address_0x1234";
        let k = 5;
        let m = 1000;

        let positions1 = compute_hash_positions(element, k, m, 0);
        let positions2 = compute_hash_positions(element, k, m, 12345);

        // Different tweaks should produce different bit positions
        assert_ne!(
            positions1, positions2,
            "Different tweaks should produce different positions for privacy rotation"
        );
    }

    #[test]
    fn test_hash_uniformity() {
        // Test that hash positions are roughly uniform across the bit array
        let m = 1000;
        let k = 7;
        let tweak = 0;
        let mut counts = vec![0usize; 10]; // 10 buckets

        for i in 0..1000 {
            let element = format!("element_{}", i);
            let positions = compute_hash_positions(element.as_bytes(), k, m, tweak);
            for pos in positions {
                let bucket = pos / 100;
                counts[bucket] += 1;
            }
        }

        // Each bucket should have roughly 1000*7/10 = 700 entries
        // Allow 50% variance for statistical tolerance
        let expected = 700;
        let min_acceptable = expected / 2;
        let max_acceptable = expected * 3 / 2;

        for (i, count) in counts.iter().enumerate() {
            assert!(
                *count >= min_acceptable && *count <= max_acceptable,
                "Bucket {} has {} entries, expected ~{} (min={}, max={})",
                i,
                count,
                expected,
                min_acceptable,
                max_acceptable
            );
        }
    }
}
