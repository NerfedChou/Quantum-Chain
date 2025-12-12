//! # GCS Filters (Golomb-Coded Sets) - BIP 158
//!
//! Privacy-preserving block filters using Golomb-Rice coding.
//!
//! ## Problem
//!
//! Standard Bloom filters (BIP 37) require clients to send their filter
//! to the node - this reveals what addresses they own.
//!
//! ## Solution: Node-Side Filters (BIP 158)
//!
//! 1. Node creates deterministic filter for every block
//! 2. Filter is served to ALL clients (same filter, zero privacy leak)
//! 3. Client downloads filter, checks locally
//!
//! ## Algorithm: Golomb-Rice Coding
//!
//! 1. Hash all output scripts in block
//! 2. Sort hashes and compute consecutive differences
//! 3. Encode differences with Golomb-Rice (unary + binary)
//! 4. Result: ~10x compression over Bloom filters
//!
//! Reference: BIP 158, SPEC-07 Phase 4

use shared_types::Hash;

/// Parameter P for Golomb-Rice coding (power of 2).
/// BIP 158 uses P = 19 for ~2^-20 collision probability.
pub const GOLOMB_P: u8 = 19;

/// M = 2^P (modulus for Golomb-Rice).
pub const GOLOMB_M: u64 = 1 << GOLOMB_P;

/// False positive rate for GCS (1 / M).
pub const GCS_FPR: f64 = 1.0 / (GOLOMB_M as f64);

/// Golomb-Coded Set filter for a single block.
///
/// ## Privacy Advantage
///
/// Unlike Bloom filters, GCS are created server-side and served
/// identically to all clients. Zero information leakage about
/// what addresses a client is watching.
#[derive(Clone, Debug)]
pub struct GcsFilter {
    /// Block hash this filter covers
    pub block_hash: Hash,
    /// Block height
    pub block_height: u64,
    /// Number of elements in the filter
    pub n: usize,
    /// Compressed filter data (Golomb-Rice encoded)
    pub data: Vec<u8>,
    /// SipHash key for deterministic hashing
    pub key: [u8; 16],
}

impl GcsFilter {
    /// Create a new GCS filter from output scripts.
    ///
    /// ## Algorithm
    /// 1. Hash each script with SipHash
    /// 2. Map to range [0, N*M)
    /// 3. Sort and compute differences
    /// 4. Golomb-Rice encode differences
    pub fn new(block_hash: Hash, block_height: u64, scripts: &[&[u8]]) -> Self {
        let n = scripts.len();
        if n == 0 {
            return Self {
                block_hash,
                block_height,
                n: 0,
                data: Vec::new(),
                key: derive_key(&block_hash),
            };
        }

        let key = derive_key(&block_hash);

        // Hash scripts and map to range
        let mut values: Vec<u64> = scripts
            .iter()
            .map(|script| hash_to_range(script, &key, n as u64 * GOLOMB_M))
            .collect();

        // Sort for delta encoding
        values.sort_unstable();

        // Compute deltas
        let mut deltas = Vec::with_capacity(n);
        let mut prev = 0u64;
        for v in &values {
            deltas.push(v.saturating_sub(prev));
            prev = *v;
        }

        // Golomb-Rice encode
        let data = golomb_encode(&deltas);

        Self {
            block_hash,
            block_height,
            n,
            data,
            key,
        }
    }

    /// Check if an element might be in the filter.
    ///
    /// Returns true if element MAY match, false if definitely not.
    pub fn match_any(&self, scripts: &[&[u8]]) -> bool {
        if self.n == 0 || scripts.is_empty() {
            return false;
        }

        // Decode filter values
        let filter_values = golomb_decode(&self.data, self.n);

        // Convert to absolute values
        let mut abs_values = Vec::with_capacity(self.n);
        let mut sum = 0u64;
        for delta in filter_values {
            sum = sum.saturating_add(delta);
            abs_values.push(sum);
        }

        // Hash query scripts
        for script in scripts {
            let h = hash_to_range(script, &self.key, self.n as u64 * GOLOMB_M);
            if abs_values.binary_search(&h).is_ok() {
                return true;
            }
        }

        false
    }

    /// Get filter size in bytes.
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Get expected FPR.
    pub fn false_positive_rate(&self) -> f64 {
        GCS_FPR
    }
}

/// Derive SipHash key from block hash (deterministic).
fn derive_key(block_hash: &Hash) -> [u8; 16] {
    let mut key = [0u8; 16];
    key.copy_from_slice(&block_hash[0..16]);
    key
}

/// Hash data to range [0, max) using SipHash-style mixing.
fn hash_to_range(data: &[u8], key: &[u8; 16], max: u64) -> u64 {
    // Simple SipHash-like mixing
    let mut state = u64::from_le_bytes(key[0..8].try_into().unwrap());
    state ^= u64::from_le_bytes(key[8..16].try_into().unwrap());

    for chunk in data.chunks(8) {
        let mut buf = [0u8; 8];
        buf[..chunk.len()].copy_from_slice(chunk);
        state ^= u64::from_le_bytes(buf);
        state = state.wrapping_mul(0x517cc1b727220a95);
        state = state.rotate_left(13);
    }

    state % max
}

/// Golomb-Rice encode a list of deltas.
fn golomb_encode(deltas: &[u64]) -> Vec<u8> {
    let mut bits = Vec::new();

    for &delta in deltas {
        let q = delta >> GOLOMB_P; // Quotient
        let r = delta & (GOLOMB_M - 1); // Remainder

        // Unary encode quotient: q ones followed by a zero
        bits.extend(std::iter::repeat_n(true, q as usize));
        bits.push(false);

        // Binary encode remainder (P bits)
        for i in (0..GOLOMB_P).rev() {
            bits.push(((r >> i) & 1) == 1);
        }
    }

    // Pack bits into bytes
    let mut bytes = Vec::with_capacity(bits.len().div_ceil(8));
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &bit) in chunk.iter().enumerate() {
            if bit {
                byte |= 1 << (7 - i);
            }
        }
        bytes.push(byte);
    }

    bytes
}

/// Golomb-Rice decode bytes back to deltas.
fn golomb_decode(data: &[u8], n: usize) -> Vec<u64> {
    let mut bits = Vec::with_capacity(data.len() * 8);
    for byte in data {
        for i in (0..8).rev() {
            bits.push((byte >> i) & 1 == 1);
        }
    }

    let mut deltas = Vec::with_capacity(n);
    let mut pos = 0;

    for _ in 0..n {
        if pos >= bits.len() {
            break;
        }

        // Decode unary quotient
        let mut q = 0u64;
        while pos < bits.len() && bits[pos] {
            q += 1;
            pos += 1;
        }
        pos += 1; // Skip the zero

        // Decode binary remainder
        let mut r = 0u64;
        for _ in 0..GOLOMB_P {
            if pos < bits.len() {
                r = (r << 1) | (bits[pos] as u64);
                pos += 1;
            }
        }

        deltas.push((q << GOLOMB_P) | r);
    }

    deltas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcs_empty() {
        let hash = [0xAB; 32];
        let filter = GcsFilter::new(hash, 100, &[]);

        assert_eq!(filter.n, 0);
        assert!(!filter.match_any(&[b"test"]));
    }

    #[test]
    fn test_gcs_single_element() {
        let hash = [0xAB; 32];
        let scripts: Vec<&[u8]> = vec![b"script1"];
        let filter = GcsFilter::new(hash, 100, &scripts);

        assert_eq!(filter.n, 1);
        assert!(filter.match_any(&[b"script1"]));
    }

    #[test]
    fn test_gcs_no_false_negatives() {
        let hash = [0xAB; 32];
        let scripts: Vec<&[u8]> = vec![b"output_script_1", b"output_script_2", b"output_script_3"];
        let filter = GcsFilter::new(hash, 100, &scripts);

        // All inserted elements must be found
        for script in &scripts {
            assert!(
                filter.match_any(&[*script]),
                "False negative for {:?}",
                script
            );
        }
    }

    #[test]
    fn test_gcs_compression() {
        let hash = [0xAB; 32];
        let scripts: Vec<Vec<u8>> = (0..100)
            .map(|i| format!("output_script_{}", i).into_bytes())
            .collect();
        let script_refs: Vec<&[u8]> = scripts.iter().map(|s| s.as_slice()).collect();

        let filter = GcsFilter::new(hash, 100, &script_refs);

        // GCS should be much smaller than raw data
        let raw_size: usize = scripts.iter().map(|s| s.len()).sum();
        assert!(filter.size_bytes() < raw_size / 2);
    }

    #[test]
    fn test_golomb_encode_decode_roundtrip() {
        let deltas: Vec<u64> = vec![500, 1000, 50, 200, 1500];
        let encoded = golomb_encode(&deltas);
        let decoded = golomb_decode(&encoded, deltas.len());

        assert_eq!(deltas, decoded);
    }

    #[test]
    fn test_gcs_fpr() {
        // FPR should be approximately 1 / 2^19
        assert!((GCS_FPR - 1.0 / 524288.0).abs() < 0.0000001);
    }
}
