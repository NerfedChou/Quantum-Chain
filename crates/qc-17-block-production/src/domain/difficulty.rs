//! Dynamic Difficulty Adjustment (DDA)
//!
//! Implements Bitcoin-style and Dark Gravity Wave (DGW) difficulty adjustment algorithms
//! to maintain consistent block times as network hashrate changes.
//!
//! **IMPORTANT**: In PoW, the "difficulty target" is actually a CEILING:
//! - HIGHER target number = EASIER (more valid hashes below it)
//! - LOWER target number = HARDER (fewer valid hashes below it)
//!
//! This is counterintuitive! When blocks are too fast, we LOWER the target.

use primitive_types::U256;
use std::time::Duration;

/// Difficulty adjustment configuration
#[derive(Clone, Debug)]
pub struct DifficultyConfig {
    /// Target time between blocks (in seconds)
    pub target_block_time: u64,

    /// Number of blocks per adjustment epoch (Bitcoin-style: 2016)
    pub adjustment_period: u64,

    /// Use Dark Gravity Wave (per-block adjustment) instead of epoch-based
    pub use_dgw: bool,

    /// Number of blocks to average for DGW (typically 24)
    pub dgw_window: usize,

    /// Initial difficulty target (genesis) - higher = easier
    pub initial_difficulty: U256,

    /// Minimum difficulty target (floor - hardest allowed)
    /// Lower number = harder, so this is the HARD limit
    pub min_difficulty: U256,

    /// Maximum difficulty target (ceiling - easiest allowed)  
    /// Higher number = easier, so this is the EASY limit
    pub max_difficulty: U256,

    /// Maximum adjustment factor per recalculation (Bitcoin uses 4x)
    /// Prevents difficulty from changing too drastically in one step
    pub max_adjustment_factor: u64,
}

impl Default for DifficultyConfig {
    fn default() -> Self {
        Self {
            target_block_time: 10,  // 10 seconds per block
            adjustment_period: 100, // Adjust every 100 blocks
            use_dgw: true,          // Enable per-block adjustment
            dgw_window: 24,         // Look at last 24 blocks
            // Initial target: realistic difficulty for single CPU mining
            // 2^235 means ~21 leading zero bits required (~2.6 zero bytes)
            // At 1M hashes/sec, this takes ~2-5 seconds
            // This prevents the "instant mining" problem on startup
            initial_difficulty: U256::from(2).pow(U256::from(235)),
            // Hardest allowed (lowest target): 2^200 (~7 leading zero bytes)
            // This prevents pools from making difficulty too hard
            min_difficulty: U256::from(2).pow(U256::from(200)),
            // Easiest allowed (highest target): 2^248 (~1 leading zero byte)
            // This prevents difficulty from becoming trivially easy
            // Changed from 2^254 to prevent near-instant mining even after reset
            max_difficulty: U256::from(2).pow(U256::from(248)),
            // Max 4x change per adjustment (Bitcoin-style)
            max_adjustment_factor: 4,
        }
    }
}

/// Block information for difficulty calculation
#[derive(Clone, Debug)]
pub struct BlockInfo {
    /// Block height
    pub height: u64,
    /// Block timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Difficulty at this block
    pub difficulty: U256,
}

/// Difficulty adjustment calculator
#[derive(Clone)]
pub struct DifficultyAdjuster {
    config: DifficultyConfig,
}

impl DifficultyAdjuster {
    /// Create a new difficulty adjuster
    pub fn new(config: DifficultyConfig) -> Self {
        Self { config }
    }

    /// Calculate next difficulty based on recent blocks
    ///
    /// # Arguments
    /// * `recent_blocks` - Recent blocks in descending order (newest first)
    ///
    /// # Returns
    /// The target difficulty for the next block
    pub fn calculate_next_difficulty(&self, recent_blocks: &[BlockInfo]) -> U256 {
        if recent_blocks.is_empty() {
            return self.config.initial_difficulty;
        }

        // If only genesis exists, use initial difficulty
        if recent_blocks.len() == 1 {
            return self.config.initial_difficulty;
        }

        if self.config.use_dgw {
            self.calculate_dgw_difficulty(recent_blocks)
        } else {
            self.calculate_epoch_difficulty(recent_blocks)
        }
    }

    /// Dark Gravity Wave: Per-block difficulty adjustment
    ///
    /// This algorithm looks at the last N blocks and adjusts difficulty based on
    /// the actual time taken vs expected time. It responds quickly to hashrate changes.
    ///
    /// REMEMBER: Target is a CEILING. Lower target = harder!
    fn calculate_dgw_difficulty(&self, recent_blocks: &[BlockInfo]) -> U256 {
        let window_size = self.config.dgw_window.min(recent_blocks.len());

        if window_size < 2 {
            return recent_blocks[0].difficulty;
        }

        // Get the blocks we'll use for calculation
        let blocks = &recent_blocks[0..window_size];

        // Calculate total time taken for these blocks
        let newest = &blocks[0];
        let oldest = &blocks[window_size - 1];

        let actual_time = if newest.timestamp > oldest.timestamp {
            newest.timestamp - oldest.timestamp
        } else {
            1 // Prevent division by zero
        };

        // Expected time for these blocks
        let expected_time = (window_size - 1) as u64 * self.config.target_block_time;

        // Prevent extreme ratios - clamp actual_time to reasonable bounds
        // This prevents the difficulty from changing too much in one step
        let min_time = expected_time / self.config.max_adjustment_factor;
        let max_time = expected_time * self.config.max_adjustment_factor;
        let clamped_actual_time = actual_time.clamp(min_time.max(1), max_time);

        // Calculate average difficulty (target) over the window
        let mut sum_difficulty = U256::zero();
        for block in blocks {
            sum_difficulty = sum_difficulty.saturating_add(block.difficulty);
        }
        let avg_difficulty = sum_difficulty / U256::from(window_size);

        // Adjust difficulty based on time ratio
        // CRITICAL: Target is a CEILING, so:
        // - Blocks too fast → LOWER the target (make it harder)
        // - Blocks too slow → RAISE the target (make it easier)
        //
        // FIX: To avoid overflow with large difficulty values, we use a different
        // approach: divide first by a common factor, then adjust.
        // new_target = old_target * (actual_time / expected_time)
        //            = old_target / expected_time * actual_time  (if divisible)
        //            OR use ratio calculation
        let new_difficulty = if expected_time > 0 {
            // Calculate ratio as fraction to avoid overflow
            // new = avg * actual / expected
            // But we need to avoid overflow, so:
            // new = (avg / expected) * actual + (avg % expected) * actual / expected
            let quotient = avg_difficulty / U256::from(expected_time);
            let remainder = avg_difficulty % U256::from(expected_time);

            quotient
                .saturating_mul(U256::from(clamped_actual_time))
                .saturating_add(
                    remainder.saturating_mul(U256::from(clamped_actual_time))
                        / U256::from(expected_time),
                )
        } else {
            avg_difficulty
        };

        // Clamp to min/max bounds
        self.clamp_difficulty(new_difficulty)
    }

    /// Bitcoin-style: Epoch-based difficulty adjustment
    ///
    /// Adjusts difficulty every N blocks based on how long those N blocks took.
    /// More predictable but slower to respond to hashrate changes.
    ///
    /// REMEMBER: Target is a CEILING. Lower target = harder!
    fn calculate_epoch_difficulty(&self, recent_blocks: &[BlockInfo]) -> U256 {
        let current_height = recent_blocks[0].height;
        let current_difficulty = recent_blocks[0].difficulty;

        // Only adjust at epoch boundaries
        if current_height % self.config.adjustment_period != 0 {
            return current_difficulty;
        }

        // Need full epoch of blocks
        if recent_blocks.len() < self.config.adjustment_period as usize {
            return current_difficulty;
        }

        let period = self.config.adjustment_period as usize;
        let epoch_blocks = &recent_blocks[0..period];

        // Calculate time taken for the epoch
        let newest = &epoch_blocks[0];
        let oldest = &epoch_blocks[period - 1];

        let actual_time = if newest.timestamp > oldest.timestamp {
            newest.timestamp - oldest.timestamp
        } else {
            self.config.target_block_time // Fallback
        };

        // Expected time for the epoch
        let expected_time = self.config.adjustment_period * self.config.target_block_time;

        // Bitcoin-style clamping: max 4x adjustment per epoch
        let min_time = expected_time / self.config.max_adjustment_factor;
        let max_time = expected_time * self.config.max_adjustment_factor;
        let clamped_actual_time = actual_time.clamp(min_time.max(1), max_time);

        // Adjust difficulty (target)
        // new_target = current_target * actual_time / expected_time
        // - If actual < expected (too fast): target decreases (harder)
        // - If actual > expected (too slow): target increases (easier)
        let new_difficulty = current_difficulty.saturating_mul(U256::from(clamped_actual_time))
            / U256::from(expected_time);

        // Clamp to min/max bounds
        self.clamp_difficulty(new_difficulty)
    }

    /// Clamp difficulty target to configured bounds
    ///
    /// Remember: min_difficulty is the HARDEST (lowest number)
    ///           max_difficulty is the EASIEST (highest number)
    fn clamp_difficulty(&self, difficulty: U256) -> U256 {
        if difficulty < self.config.min_difficulty {
            // Target too low (too hard) - clamp to hardest allowed
            self.config.min_difficulty
        } else if difficulty > self.config.max_difficulty {
            // Target too high (too easy) - clamp to easiest allowed
            self.config.max_difficulty
        } else {
            difficulty
        }
    }

    /// Get a human-readable description of the difficulty
    pub fn describe_difficulty(difficulty: U256) -> String {
        // Count leading zero bits
        let leading_zeros = difficulty.leading_zeros();
        let leading_zero_bytes = leading_zeros / 8;

        format!(
            "~{} leading zero bytes ({})",
            leading_zero_bytes, difficulty
        )
    }

    /// Calculate expected hashrate based on difficulty and block time
    pub fn estimate_hashrate(&self, difficulty: U256, actual_block_time: Duration) -> f64 {
        // Rough estimate: hashrate = difficulty / time
        let time_secs = actual_block_time.as_secs_f64();
        if time_secs == 0.0 {
            return 0.0;
        }

        // Convert difficulty to f64 (approximation)
        let diff_f64 = difficulty.low_u128() as f64;
        diff_f64 / time_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_difficulty() {
        let config = DifficultyConfig::default();
        let adjuster = DifficultyAdjuster::new(config.clone());

        let difficulty = adjuster.calculate_next_difficulty(&[]);
        assert_eq!(difficulty, config.initial_difficulty);
    }

    #[test]
    fn test_dgw_lowers_target_for_fast_blocks() {
        let config = DifficultyConfig {
            target_block_time: 10,
            dgw_window: 3,
            max_adjustment_factor: 4,
            ..Default::default()
        };
        let adjuster = DifficultyAdjuster::new(config.clone());

        // Simulate blocks being mined too fast (5 seconds each instead of 10)
        // 3 blocks, 2 intervals, 10 seconds total (should be 20)
        let blocks = vec![
            BlockInfo {
                height: 3,
                timestamp: 30,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 2,
                timestamp: 25,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 1,
                timestamp: 20,
                difficulty: config.initial_difficulty,
            },
        ];

        let new_difficulty = adjuster.calculate_next_difficulty(&blocks);

        // Target should DECREASE (lower number = harder to hit)
        // Because blocks were mined 2x too fast
        assert!(
            new_difficulty < config.initial_difficulty,
            "Fast blocks should lower target. Got {} vs initial {}",
            new_difficulty,
            config.initial_difficulty
        );
    }

    #[test]
    fn test_dgw_raises_target_for_slow_blocks() {
        let config = DifficultyConfig {
            target_block_time: 10,
            dgw_window: 3,
            max_adjustment_factor: 4,
            ..Default::default()
        };
        let adjuster = DifficultyAdjuster::new(config.clone());

        // Simulate blocks being mined too slow (20 seconds each instead of 10)
        // 3 blocks, 2 intervals, 40 seconds total (should be 20)
        let blocks = vec![
            BlockInfo {
                height: 3,
                timestamp: 60,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 2,
                timestamp: 40,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 1,
                timestamp: 20,
                difficulty: config.initial_difficulty,
            },
        ];

        let new_difficulty = adjuster.calculate_next_difficulty(&blocks);

        // Target should INCREASE (higher number = easier to hit)
        // Because blocks were mined 2x too slow
        assert!(
            new_difficulty > config.initial_difficulty,
            "Slow blocks should raise target. Got {} vs initial {}",
            new_difficulty,
            config.initial_difficulty
        );
    }

    #[test]
    fn test_max_adjustment_factor_limits_change() {
        let config = DifficultyConfig {
            target_block_time: 10,
            dgw_window: 3,
            max_adjustment_factor: 4,
            ..Default::default()
        };
        let adjuster = DifficultyAdjuster::new(config.clone());

        // Simulate EXTREMELY fast blocks (0.1 seconds each - 100x too fast)
        // Without clamping, this would make target 100x smaller
        // With 4x clamp, it should only be 4x smaller
        let blocks = vec![
            BlockInfo {
                height: 3,
                timestamp: 1002,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 2,
                timestamp: 1001,
                difficulty: config.initial_difficulty,
            },
            BlockInfo {
                height: 1,
                timestamp: 1000,
                difficulty: config.initial_difficulty,
            },
        ];

        let new_difficulty = adjuster.calculate_next_difficulty(&blocks);

        // Should be clamped to at most 4x harder (target / 4)
        let max_decrease = config.initial_difficulty / U256::from(4);
        assert!(
            new_difficulty >= max_decrease,
            "Adjustment should be clamped. Got {} but min should be {}",
            new_difficulty,
            max_decrease
        );
    }

    #[test]
    fn test_clamping_bounds() {
        let config = DifficultyConfig::default();
        let adjuster = DifficultyAdjuster::new(config.clone());

        // Test min clamping (hardest allowed - lowest number)
        let too_low = config.min_difficulty / U256::from(2);
        assert_eq!(adjuster.clamp_difficulty(too_low), config.min_difficulty);

        // Test max clamping (easiest allowed - highest number)
        let too_high = config.max_difficulty * U256::from(2);
        assert_eq!(adjuster.clamp_difficulty(too_high), config.max_difficulty);
    }
}
