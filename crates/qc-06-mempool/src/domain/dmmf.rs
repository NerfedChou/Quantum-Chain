//! # Dynamic Minimum Mempool Fee (DMMF)
//!
//! Implements exponential-decay fee floor for congestion management.
//!
//! ## Problem
//!
//! Static size limits reject ALL transactions when mempool is full.
//! This is economically inefficient - high-value transactions get rejected.
//!
//! ## Solution: Exponential-Decay-Fee-Floor
//!
//! 1. When pool fills, evict lowest feerate transaction
//! 2. Raise min_fee to evicted tx's feerate + epsilon
//! 3. Decay min_fee every block (multiply by 0.5)
//!
//! ## Security
//!
//! Prevents spam attacks from filling mempool with cheap garbage.

use super::U256;
use std::time::Instant;

/// Dynamic minimum fee manager.
///
/// ## Algorithm: Exponential-Decay-Fee-Floor
///
/// - Trigger: When mempool exceeds max size
/// - Action: Evict lowest feerate, raise floor
/// - Decay: Halve min_fee every block/10min
#[derive(Debug)]
pub struct DynamicMinFee {
    /// Current minimum acceptance fee (wei per gas)
    min_fee: U256,
    /// Base minimum fee (default floor)
    base_fee: U256,
    /// Last decay timestamp
    last_decay: Instant,
    /// Decay interval (milliseconds)
    decay_interval_ms: u64,
    /// Decay factor (0.5 = halve each interval)
    decay_factor: f64,
    /// Fee bump epsilon for new floor
    fee_bump: U256,
}

/// Default decay interval (10 minutes).
pub const DEFAULT_DECAY_INTERVAL_MS: u64 = 600_000;

/// Default decay factor (50% per interval).
pub const DEFAULT_DECAY_FACTOR: f64 = 0.5;

/// Default fee bump epsilon.
pub const DEFAULT_FEE_BUMP: u64 = 1_000_000; // 0.001 gwei

impl DynamicMinFee {
    /// Create new DMMF with base minimum fee.
    pub fn new(base_fee: U256) -> Self {
        Self {
            min_fee: base_fee,
            base_fee,
            last_decay: Instant::now(),
            decay_interval_ms: DEFAULT_DECAY_INTERVAL_MS,
            decay_factor: DEFAULT_DECAY_FACTOR,
            fee_bump: U256::from(DEFAULT_FEE_BUMP),
        }
    }

    /// Create with custom parameters.
    pub fn with_params(
        base_fee: U256,
        decay_interval_ms: u64,
        decay_factor: f64,
        fee_bump: U256,
    ) -> Self {
        Self {
            min_fee: base_fee,
            base_fee,
            last_decay: Instant::now(),
            decay_interval_ms,
            decay_factor,
            fee_bump,
        }
    }

    /// Get current minimum fee.
    pub fn current_min_fee(&self) -> U256 {
        self.min_fee
    }

    /// Check if a transaction meets the minimum fee.
    pub fn meets_minimum(&self, gas_price: U256) -> bool {
        gas_price >= self.min_fee
    }

    /// Raise the fee floor after eviction.
    ///
    /// Called when evicting the lowest-feerate transaction.
    pub fn raise_floor(&mut self, evicted_feerate: U256) {
        self.min_fee = evicted_feerate + self.fee_bump;
        self.last_decay = Instant::now();
    }

    /// Apply time-based decay to the fee floor.
    ///
    /// Should be called periodically (e.g., on new block).
    pub fn apply_decay(&mut self) {
        let elapsed = self.last_decay.elapsed().as_millis() as u64;
        let intervals = elapsed / self.decay_interval_ms;

        if intervals > 0 {
            // Apply decay: min_fee *= decay_factor^intervals
            for _ in 0..intervals.min(10) {
                let decayed = u256_mul_f64(self.min_fee, self.decay_factor);
                self.min_fee = decayed;
            }

            // Never go below base fee
            if self.min_fee < self.base_fee {
                self.min_fee = self.base_fee;
            }

            self.last_decay = Instant::now();
        }
    }

    /// Reset to base fee (e.g., after mempool clears).
    pub fn reset(&mut self) {
        self.min_fee = self.base_fee;
        self.last_decay = Instant::now();
    }

    /// Get statistics for metrics.
    pub fn stats(&self) -> DynamicFeeStats {
        DynamicFeeStats {
            current_min_fee: self.min_fee,
            base_fee: self.base_fee,
            multiplier: if self.base_fee > U256::zero() {
                u256_div_ratio(self.min_fee, self.base_fee)
            } else {
                1.0
            },
        }
    }
}

/// Statistics for monitoring.
#[derive(Clone, Debug)]
pub struct DynamicFeeStats {
    pub current_min_fee: U256,
    pub base_fee: U256,
    pub multiplier: f64,
}

/// Multiply U256 by float (for decay).
fn u256_mul_f64(value: U256, factor: f64) -> U256 {
    // Use fixed-point arithmetic: multiply by 1000, then divide
    let factor_fixed = (factor * 1000.0) as u64;
    (value * U256::from(factor_fixed)) / U256::from(1000u64)
}

/// Divide U256 for ratio (for stats).
fn u256_div_ratio(numerator: U256, denominator: U256) -> f64 {
    if denominator == U256::zero() {
        return 1.0;
    }
    // Simple approximation for reasonable values
    let num_low = numerator.low_u64() as f64;
    let den_low = denominator.low_u64() as f64;
    if den_low > 0.0 {
        num_low / den_low
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_min_fee() {
        let dmmf = DynamicMinFee::new(U256::from(1_000_000_000u64));
        assert_eq!(dmmf.current_min_fee(), U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_meets_minimum() {
        let dmmf = DynamicMinFee::new(U256::from(1_000_000_000u64));
        
        assert!(dmmf.meets_minimum(U256::from(1_000_000_000u64)));
        assert!(dmmf.meets_minimum(U256::from(2_000_000_000u64)));
        assert!(!dmmf.meets_minimum(U256::from(999_999_999u64)));
    }

    #[test]
    fn test_raise_floor() {
        let mut dmmf = DynamicMinFee::new(U256::from(1_000_000_000u64));
        
        dmmf.raise_floor(U256::from(5_000_000_000u64));
        
        // Should be evicted + bump
        assert!(dmmf.current_min_fee() > U256::from(5_000_000_000u64));
    }

    #[test]
    fn test_reset() {
        let mut dmmf = DynamicMinFee::new(U256::from(1_000_000_000u64));
        
        dmmf.raise_floor(U256::from(10_000_000_000u64));
        assert!(dmmf.current_min_fee() > U256::from(1_000_000_000u64));
        
        dmmf.reset();
        assert_eq!(dmmf.current_min_fee(), U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_stats() {
        let dmmf = DynamicMinFee::new(U256::from(1_000_000_000u64));
        let stats = dmmf.stats();
        
        assert_eq!(stats.base_fee, U256::from(1_000_000_000u64));
        assert!((stats.multiplier - 1.0).abs() < 0.01);
    }
}
