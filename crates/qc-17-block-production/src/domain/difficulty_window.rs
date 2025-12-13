//! Difficulty Window Calculator for DGW (Dark Gravity Wave) algorithm.
//!
//! This module handles the business logic for calculating mining difficulty
//! based on recent block history. Belongs in domain layer (no I/O).

use primitive_types::U256;

/// Block information needed for difficulty calculation.
#[derive(Debug, Clone, Copy)]
pub struct BlockDifficultyInfo {
    /// Block height
    pub height: u64,
    /// Block timestamp (Unix epoch milliseconds)
    pub timestamp: u64,
    /// Block difficulty target
    pub difficulty: U256,
}

/// Configuration for the DGW difficulty window.
#[derive(Debug, Clone)]
pub struct DifficultyWindowConfig {
    /// Maximum window size for DGW calculation
    pub max_window_size: usize,
    /// Target block time in milliseconds
    pub target_block_time_ms: u64,
}

impl Default for DifficultyWindowConfig {
    fn default() -> Self {
        Self {
            max_window_size: 24,
            target_block_time_ms: 150_000, // 2.5 minutes
        }
    }
}

/// Calculator for Dark Gravity Wave difficulty adjustment.
pub struct DifficultyWindowCalculator {
    config: DifficultyWindowConfig,
}

impl DifficultyWindowCalculator {
    /// Create a new difficulty window calculator.
    pub fn new(config: DifficultyWindowConfig) -> Self {
        Self { config }
    }

    /// Calculate the effective window size based on chain height.
    pub fn calculate_window_size(&self, chain_height: u64) -> usize {
        self.config.max_window_size.min(chain_height as usize)
    }

    /// Calculate the start height for loading block history.
    pub fn calculate_start_height(&self, chain_height: u64) -> u64 {
        let window_size = self.calculate_window_size(chain_height);
        chain_height.saturating_sub(window_size as u64)
    }

    /// Resolve difficulty from a block, using fallback if block has zero difficulty.
    pub fn resolve_difficulty(&self, block_difficulty: U256, fallback: U256) -> U256 {
        if block_difficulty.is_zero() {
            fallback
        } else {
            block_difficulty
        }
    }

    /// Calculate new difficulty based on block history (DGW algorithm).
    ///
    /// # Arguments
    /// * `blocks` - Recent blocks in newest-first order
    ///
    /// # Returns
    /// The calculated difficulty for the next block
    pub fn calculate_next_difficulty(&self, blocks: &[BlockDifficultyInfo]) -> U256 {
        if blocks.is_empty() {
            return U256::from(1);
        }

        if blocks.len() < 2 {
            return blocks[0].difficulty;
        }

        // DGW uses weighted average of recent block times
        let window_size = blocks.len();
        let mut weighted_sum = U256::zero();
        let mut weight_total = U256::zero();

        for (i, block) in blocks.iter().enumerate() {
            let weight = U256::from(window_size - i);
            weighted_sum += block.difficulty * weight;
            weight_total += weight;
        }

        if weight_total.is_zero() {
            return blocks[0].difficulty;
        }

        weighted_sum / weight_total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_size_calculation() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        assert_eq!(calc.calculate_window_size(10), 10);
        assert_eq!(calc.calculate_window_size(24), 24);
        assert_eq!(calc.calculate_window_size(100), 24);
    }

    #[test]
    fn test_start_height_calculation() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        assert_eq!(calc.calculate_start_height(10), 0);
        assert_eq!(calc.calculate_start_height(24), 0);
        assert_eq!(calc.calculate_start_height(100), 76);
    }

    #[test]
    fn test_resolve_difficulty_with_zero() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());
        let fallback = U256::from(1000);

        assert_eq!(calc.resolve_difficulty(U256::zero(), fallback), fallback);
        assert_eq!(
            calc.resolve_difficulty(U256::from(500), fallback),
            U256::from(500)
        );
    }

    #[test]
    fn test_calculate_next_difficulty_empty() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        assert_eq!(calc.calculate_next_difficulty(&[]), U256::from(1));
    }

    #[test]
    fn test_calculate_next_difficulty_single_block() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());
        let block = BlockDifficultyInfo {
            height: 1,
            timestamp: 1000,
            difficulty: U256::from(100),
        };

        assert_eq!(calc.calculate_next_difficulty(&[block]), U256::from(100));
    }

    #[test]
    fn test_calculate_next_difficulty_weighted_average() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        // Three blocks with different difficulties
        // Weighted average should give more weight to newer blocks
        let blocks = vec![
            BlockDifficultyInfo {
                height: 3,
                timestamp: 3000,
                difficulty: U256::from(300),
            },
            BlockDifficultyInfo {
                height: 2,
                timestamp: 2000,
                difficulty: U256::from(200),
            },
            BlockDifficultyInfo {
                height: 1,
                timestamp: 1000,
                difficulty: U256::from(100),
            },
        ];

        let result = calc.calculate_next_difficulty(&blocks);

        // Weighted average: (300*3 + 200*2 + 100*1) / (3+2+1) = 1400 / 6 = 233.33... = 233
        assert_eq!(result, U256::from(233));
    }

    #[test]
    fn test_calculate_next_difficulty_equal_weights() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        // Two blocks with same difficulty should return that difficulty
        let blocks = vec![
            BlockDifficultyInfo {
                height: 2,
                timestamp: 2000,
                difficulty: U256::from(200),
            },
            BlockDifficultyInfo {
                height: 1,
                timestamp: 1000,
                difficulty: U256::from(200),
            },
        ];

        let result = calc.calculate_next_difficulty(&blocks);

        // (200*2 + 200*1) / (2+1) = 600 / 3 = 200
        assert_eq!(result, U256::from(200));
    }

    #[test]
    fn test_window_size_zero_height() {
        let calc = DifficultyWindowCalculator::new(DifficultyWindowConfig::default());

        assert_eq!(calc.calculate_window_size(0), 0);
        assert_eq!(calc.calculate_start_height(0), 0);
    }

    #[test]
    fn test_custom_window_config() {
        let config = DifficultyWindowConfig {
            max_window_size: 10,
            target_block_time_ms: 60_000,
        };
        let calc = DifficultyWindowCalculator::new(config);

        assert_eq!(calc.calculate_window_size(5), 5);
        assert_eq!(calc.calculate_window_size(10), 10);
        assert_eq!(calc.calculate_window_size(20), 10);
    }
}
