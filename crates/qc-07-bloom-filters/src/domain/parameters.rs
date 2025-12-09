//! Optimal Bloom filter parameter calculation
//!
//! Reference: System.md, Subsystem 7 - FPR = (1 - e^(-kn/m))^k
//!
//! Formulas:
//! - m = -n*ln(fpr) / (ln(2)^2)  -- optimal bits
//! - k = (m/n) * ln(2)           -- optimal hash functions

use std::f64::consts::LN_2;

/// Bloom filter parameters
#[derive(Clone, Debug, PartialEq)]
pub struct BloomFilterParams {
    /// Number of bits in the filter
    pub size_bits: usize,
    /// Number of hash functions
    pub hash_count: usize,
    /// Expected false positive rate with these parameters
    pub expected_fpr: f64,
}

/// Calculate optimal Bloom filter parameters for given constraints
///
/// # Arguments
/// * `num_elements` - Expected number of elements to insert (n)
/// * `target_fpr` - Target false positive rate
///
/// # Returns
/// Optimal parameters (m, k) that achieve the target FPR
///
/// # Reference
/// System.md, Subsystem 7:
/// - FPR = (1 - e^(-kn/m))^k
/// - m = -n*ln(fpr) / (ln(2)^2)
/// - k = (m/n) * ln(2)
pub fn calculate_optimal_parameters(num_elements: usize, target_fpr: f64) -> BloomFilterParams {
    if num_elements == 0 {
        return BloomFilterParams {
            size_bits: 1,
            hash_count: 1,
            expected_fpr: 1.0,
        };
    }

    let n = num_elements as f64;
    let ln2_squared = LN_2 * LN_2;

    // Optimal number of bits: m = -n * ln(fpr) / (ln(2)^2)
    let m = (-n * target_fpr.ln() / ln2_squared).ceil() as usize;

    // Optimal number of hash functions: k = (m/n) * ln(2)
    let k = ((m as f64 / n) * LN_2).round() as usize;
    let k = k.clamp(1, 32); // Clamp to reasonable range

    // Calculate actual FPR with these parameters
    let expected_fpr = calculate_fpr(m, num_elements, k);

    BloomFilterParams {
        size_bits: m,
        hash_count: k,
        expected_fpr,
    }
}

/// Calculate the false positive rate for given parameters
///
/// Formula: FPR = (1 - e^(-kn/m))^k
pub fn calculate_fpr(m: usize, n: usize, k: usize) -> f64 {
    if m == 0 {
        return 1.0;
    }
    let exponent = -(k as f64) * (n as f64) / (m as f64);
    (1.0 - exponent.exp()).powi(k as i32)
}

/// Calculate optimal k for given m and n
pub fn optimal_k(m: usize, n: usize) -> usize {
    if n == 0 {
        return 1;
    }
    ((m as f64 / n as f64) * LN_2).round() as usize
}

/// Calculate minimum m for given n and target FPR
pub fn minimum_bits(n: usize, target_fpr: f64) -> usize {
    let ln2_squared = LN_2 * LN_2;
    (-(n as f64) * target_fpr.ln() / ln2_squared).ceil() as usize
}

/// Density-Adaptive FPR Auto-Tuning.
///
/// Dynamically adjusts filter parameters based on block density.
///
/// ## Algorithm
///
/// - If block has < SKIP_THRESHOLD transactions, return None (skip filter)
/// - If block is dense, increase bits to maintain FPR
/// - If block is sparse, reduce bits to save bandwidth
///
/// ## Reference
/// 
/// SPEC-07 Phase 4 - False Positive Auto-Tuning
pub struct AdaptiveBloomParams {
    /// Target FPR
    pub target_fpr: f64,
    /// Average transactions per block (for normalization)
    pub avg_block_size: usize,
    /// Below this, skip filter entirely
    pub skip_threshold: usize,
    /// Maximum bits (cap for dense blocks)
    pub max_bits: usize,
}

impl AdaptiveBloomParams {
    /// Create with defaults.
    pub fn new(target_fpr: f64, avg_block_size: usize) -> Self {
        Self {
            target_fpr,
            avg_block_size,
            skip_threshold: 10,
            max_bits: 100_000,
        }
    }

    /// Calculate parameters for a specific block.
    ///
    /// Returns None if filter should be skipped (send raw hashes instead).
    pub fn for_block(&self, tx_count: usize) -> Option<BloomFilterParams> {
        if tx_count < self.skip_threshold {
            return None; // Skip filter, send raw hashes
        }

        let mut params = calculate_optimal_parameters(tx_count, self.target_fpr);

        // Cap maximum bits
        if params.size_bits > self.max_bits {
            params.size_bits = self.max_bits;
            params.expected_fpr = calculate_fpr(params.size_bits, tx_count, params.hash_count);
        }

        Some(params)
    }

    /// Estimate bandwidth savings vs raw transactions.
    ///
    /// Positive = filter is smaller, negative = raw is smaller.
    pub fn bandwidth_savings(&self, tx_count: usize, avg_tx_hash_size: usize) -> i64 {
        match self.for_block(tx_count) {
            None => 0, // Skip case - same size
            Some(params) => {
                let filter_bytes = params.size_bits / 8;
                let raw_bytes = tx_count * avg_tx_hash_size;
                raw_bytes as i64 - filter_bytes as i64
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_parameters_n50_fpr0001() {
        // For n=50, FPR=0.0001 → expect k≈13, m≈959
        let params = calculate_optimal_parameters(50, 0.0001);

        assert!(
            params.hash_count >= 10 && params.hash_count <= 15,
            "Expected k≈13, got k={}",
            params.hash_count
        );
        assert!(
            params.size_bits >= 800 && params.size_bits <= 1200,
            "Expected m≈959, got m={}",
            params.size_bits
        );
    }

    #[test]
    fn test_optimal_parameters_n100_fpr001() {
        // For n=100, FPR=0.01 → expect k≈7, m≈959
        let params = calculate_optimal_parameters(100, 0.01);

        assert!(
            params.hash_count >= 5 && params.hash_count <= 9,
            "Expected k≈7, got k={}",
            params.hash_count
        );
        assert!(
            params.size_bits >= 800 && params.size_bits <= 1200,
            "Expected m≈959, got m={}",
            params.size_bits
        );
    }

    #[test]
    fn test_fpr_calculation() {
        // With m=1000, n=100, k=7, FPR should be around 0.008
        let fpr = calculate_fpr(1000, 100, 7);
        assert!(fpr > 0.005 && fpr < 0.02, "Expected FPR≈0.008, got {}", fpr);
    }

    #[test]
    fn test_expected_fpr_meets_target() {
        let target_fpr = 0.01;
        let params = calculate_optimal_parameters(100, target_fpr);

        // Expected FPR should be at or below target
        assert!(
            params.expected_fpr <= target_fpr * 1.1, // Allow 10% tolerance
            "Expected FPR {} should be <= target {}",
            params.expected_fpr,
            target_fpr
        );
    }

    #[test]
    fn test_zero_elements() {
        let params = calculate_optimal_parameters(0, 0.01);
        assert_eq!(params.size_bits, 1);
        assert_eq!(params.hash_count, 1);
    }

    #[test]
    fn test_k_clamped_to_reasonable_range() {
        // Very small FPR would need many hash functions
        let params = calculate_optimal_parameters(10, 0.0000001);
        assert!(params.hash_count <= 32, "k should be clamped to max 32");
        assert!(params.hash_count >= 1, "k should be at least 1");
    }

    #[test]
    fn test_larger_n_needs_more_bits() {
        let params1 = calculate_optimal_parameters(100, 0.01);
        let params2 = calculate_optimal_parameters(1000, 0.01);

        assert!(
            params2.size_bits > params1.size_bits,
            "More elements should need more bits"
        );
    }

    #[test]
    fn test_lower_fpr_needs_more_bits() {
        let params1 = calculate_optimal_parameters(100, 0.1);
        let params2 = calculate_optimal_parameters(100, 0.01);

        assert!(
            params2.size_bits > params1.size_bits,
            "Lower FPR should need more bits"
        );
    }
}
