//! Block Validation Service - Pure Domain Logic
//!
//! This module contains the core validation rules for blocks.
//! All logic is pure (no I/O, no async) following DDD principles.
//!
//! Reference: SPEC-08-CONSENSUS.md Section 4

use primitive_types::U256;
use std::collections::HashSet;

/// Parameters for block validation.
#[derive(Debug, Clone)]
pub struct BlockValidationParams {
    /// Block hash.
    pub block_hash: [u8; 32],
    /// Block height.
    pub block_height: u64,
    /// Difficulty target as big-endian bytes.
    pub difficulty: [u8; 32],
    /// PoW nonce.
    pub nonce: u64,
    /// Block timestamp (Unix epoch seconds).
    pub timestamp: u64,
    /// Parent block hash.
    pub parent_hash: [u8; 32],
}

/// Configuration for block validation.
#[derive(Debug, Clone)]
pub struct BlockValidationConfig {
    /// Maximum number of validated blocks to cache (for duplicate detection).
    pub max_cached_blocks: usize,
    /// Maximum timestamp drift allowed into the future (seconds).
    pub max_future_drift_secs: u64,
    /// Whether to enforce strict sequential height validation.
    pub strict_height_validation: bool,
}

impl Default for BlockValidationConfig {
    fn default() -> Self {
        Self {
            max_cached_blocks: 1000,
            max_future_drift_secs: 15,
            strict_height_validation: false, // Allow flexibility during initial sync
        }
    }
}

/// Block validation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockValidationError {
    /// Block has already been validated.
    DuplicateBlock { block_hash: [u8; 32] },
    /// Block height is not sequential.
    NonSequentialHeight { expected: u64, got: u64 },
    /// Block has zero difficulty (invalid).
    ZeroDifficulty,
    /// Block timestamp is too far in the future.
    FutureTimestamp {
        block_timestamp: u64,
        current_time: u64,
        max_drift: u64,
    },
}

impl std::fmt::Display for BlockValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateBlock { block_hash } => {
                write!(
                    f,
                    "Block already validated: {:02x}{:02x}...",
                    block_hash[0], block_hash[1]
                )
            }
            Self::NonSequentialHeight { expected, got } => {
                write!(
                    f,
                    "Non-sequential block height: expected {}, got {}",
                    expected, got
                )
            }
            Self::ZeroDifficulty => write!(f, "Block has zero difficulty"),
            Self::FutureTimestamp {
                block_timestamp,
                current_time,
                max_drift,
            } => {
                write!(
                    f,
                    "Block timestamp {} is {} seconds in the future (max allowed: {})",
                    block_timestamp,
                    block_timestamp.saturating_sub(*current_time),
                    max_drift
                )
            }
        }
    }
}

impl std::error::Error for BlockValidationError {}

/// Result of successful block validation.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The validated block hash.
    pub block_hash: [u8; 32],
    /// The validated block height.
    pub block_height: u64,
    /// Any warnings generated during validation.
    pub warnings: Vec<ValidationWarning>,
}

/// Non-fatal validation warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationWarning {
    /// Block timestamp is slightly in the future (but within tolerance).
    SlightlyFutureTimestamp { block_timestamp: u64, current_time: u64 },
    /// Block height was non-sequential but allowed (during sync).
    NonSequentialHeightAllowed { expected: u64, got: u64 },
}

/// Pure domain service for block validation.
///
/// This service implements the core validation rules without any I/O.
/// State management (caching, chain height) is handled externally.
pub struct BlockValidator {
    config: BlockValidationConfig,
}

impl BlockValidator {
    /// Create a new block validator with the given configuration.
    pub fn new(config: BlockValidationConfig) -> Self {
        Self { config }
    }

    /// Create a new block validator with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(BlockValidationConfig::default())
    }

    /// Check if a block is a duplicate (already in the validated set).
    pub fn check_duplicate(
        &self,
        block_hash: &[u8; 32],
        validated_blocks: &HashSet<[u8; 32]>,
    ) -> Result<(), BlockValidationError> {
        if validated_blocks.contains(block_hash) {
            Err(BlockValidationError::DuplicateBlock {
                block_hash: *block_hash,
            })
        } else {
            Ok(())
        }
    }

    /// Validate block height is sequential.
    ///
    /// Returns Ok with optional warning if validation passes.
    pub fn validate_height(
        &self,
        block_height: u64,
        current_chain_height: u64,
    ) -> Result<Option<ValidationWarning>, BlockValidationError> {
        let expected_height = current_chain_height + 1;

        // Allow genesis block (height 0) and first block (height 1)
        if block_height <= 1 {
            return Ok(None);
        }

        if block_height != expected_height {
            if self.config.strict_height_validation {
                return Err(BlockValidationError::NonSequentialHeight {
                    expected: expected_height,
                    got: block_height,
                });
            } else {
                // Non-strict mode: warn but allow
                return Ok(Some(ValidationWarning::NonSequentialHeightAllowed {
                    expected: expected_height,
                    got: block_height,
                }));
            }
        }

        Ok(None)
    }

    /// Validate PoW difficulty is non-zero.
    pub fn validate_difficulty(&self, difficulty: &[u8; 32]) -> Result<(), BlockValidationError> {
        let difficulty_u256 = U256::from_big_endian(difficulty);
        if difficulty_u256.is_zero() {
            Err(BlockValidationError::ZeroDifficulty)
        } else {
            Ok(())
        }
    }

    /// Validate block timestamp is not too far in the future.
    ///
    /// Returns Ok with optional warning if validation passes.
    pub fn validate_timestamp(
        &self,
        block_timestamp: u64,
        current_time: u64,
    ) -> Result<Option<ValidationWarning>, BlockValidationError> {
        let max_allowed = current_time + self.config.max_future_drift_secs;

        if block_timestamp > max_allowed {
            return Err(BlockValidationError::FutureTimestamp {
                block_timestamp,
                current_time,
                max_drift: self.config.max_future_drift_secs,
            });
        }

        // Warn if timestamp is in the future at all (but within tolerance)
        if block_timestamp > current_time {
            return Ok(Some(ValidationWarning::SlightlyFutureTimestamp {
                block_timestamp,
                current_time,
            }));
        }

        Ok(None)
    }

    /// Check if the validated block cache needs eviction.
    pub fn should_evict_cache(&self, cache_size: usize) -> bool {
        cache_size > self.config.max_cached_blocks
    }

    /// Perform full block validation.
    ///
    /// # Arguments
    /// * `params` - Block validation parameters
    /// * `current_chain_height` - Current height of the validated chain
    /// * `current_time` - Current Unix timestamp in seconds
    /// * `validated_blocks` - Set of already validated block hashes
    ///
    /// # Returns
    /// * `Ok(ValidationResult)` - Block is valid (may include warnings)
    /// * `Err(BlockValidationError)` - Block is invalid
    pub fn validate_block(
        &self,
        params: &BlockValidationParams,
        current_chain_height: u64,
        current_time: u64,
        validated_blocks: &HashSet<[u8; 32]>,
    ) -> Result<ValidationResult, BlockValidationError> {
        let mut warnings = Vec::new();

        // 1. Check for duplicate
        self.check_duplicate(&params.block_hash, validated_blocks)?;

        // 2. Validate height
        if let Some(warning) = self.validate_height(params.block_height, current_chain_height)? {
            warnings.push(warning);
        }

        // 3. Validate difficulty
        self.validate_difficulty(&params.difficulty)?;

        // 4. Validate timestamp
        if let Some(warning) = self.validate_timestamp(params.timestamp, current_time)? {
            warnings.push(warning);
        }

        Ok(ValidationResult {
            block_hash: params.block_hash,
            block_height: params.block_height,
            warnings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_params(height: u64, timestamp: u64) -> BlockValidationParams {
        BlockValidationParams {
            block_hash: [height as u8; 32],
            block_height: height,
            difficulty: {
                let mut d = [0u8; 32];
                d[0] = 1; // Non-zero difficulty
                d
            },
            nonce: 12345,
            timestamp,
            parent_hash: [(height.saturating_sub(1)) as u8; 32],
        }
    }

    #[test]
    fn test_duplicate_detection() {
        let validator = BlockValidator::with_defaults();
        let block_hash = [1u8; 32];

        let mut validated = HashSet::new();

        // First time: not a duplicate
        assert!(validator.check_duplicate(&block_hash, &validated).is_ok());

        // Add to validated set
        validated.insert(block_hash);

        // Second time: duplicate
        let result = validator.check_duplicate(&block_hash, &validated);
        assert!(matches!(
            result,
            Err(BlockValidationError::DuplicateBlock { .. })
        ));
    }

    #[test]
    fn test_sequential_height_validation() {
        let validator = BlockValidator::with_defaults();

        // Genesis block (height 0) always valid
        assert!(validator.validate_height(0, 0).unwrap().is_none());

        // First block (height 1) always valid
        assert!(validator.validate_height(1, 0).unwrap().is_none());

        // Sequential block valid
        assert!(validator.validate_height(5, 4).unwrap().is_none());

        // Non-sequential block (warning in non-strict mode)
        let result = validator.validate_height(10, 4).unwrap();
        assert!(matches!(
            result,
            Some(ValidationWarning::NonSequentialHeightAllowed { .. })
        ));
    }

    #[test]
    fn test_strict_height_validation() {
        let config = BlockValidationConfig {
            strict_height_validation: true,
            ..Default::default()
        };
        let validator = BlockValidator::new(config);

        // Sequential block valid
        assert!(validator.validate_height(5, 4).is_ok());

        // Non-sequential block (error in strict mode)
        let result = validator.validate_height(10, 4);
        assert!(matches!(
            result,
            Err(BlockValidationError::NonSequentialHeight { .. })
        ));
    }

    #[test]
    fn test_zero_difficulty_rejected() {
        let validator = BlockValidator::with_defaults();

        let zero_difficulty = [0u8; 32];
        assert!(matches!(
            validator.validate_difficulty(&zero_difficulty),
            Err(BlockValidationError::ZeroDifficulty)
        ));

        let nonzero_difficulty = {
            let mut d = [0u8; 32];
            d[31] = 1;
            d
        };
        assert!(validator.validate_difficulty(&nonzero_difficulty).is_ok());
    }

    #[test]
    fn test_future_timestamp_rejected() {
        let validator = BlockValidator::with_defaults();
        let current_time = 1000;

        // Past timestamp: valid, no warning
        let result = validator.validate_timestamp(900, current_time).unwrap();
        assert!(result.is_none());

        // Current timestamp: valid, no warning
        let result = validator.validate_timestamp(1000, current_time).unwrap();
        assert!(result.is_none());

        // Slightly future (within drift): valid with warning
        let result = validator.validate_timestamp(1010, current_time).unwrap();
        assert!(matches!(
            result,
            Some(ValidationWarning::SlightlyFutureTimestamp { .. })
        ));

        // Too far in future: rejected
        let result = validator.validate_timestamp(1100, current_time);
        assert!(matches!(
            result,
            Err(BlockValidationError::FutureTimestamp { .. })
        ));
    }

    #[test]
    fn test_cache_eviction_check() {
        let config = BlockValidationConfig {
            max_cached_blocks: 100,
            ..Default::default()
        };
        let validator = BlockValidator::new(config);

        assert!(!validator.should_evict_cache(50));
        assert!(!validator.should_evict_cache(100));
        assert!(validator.should_evict_cache(101));
        assert!(validator.should_evict_cache(200));
    }

    #[test]
    fn test_full_validation_success() {
        let validator = BlockValidator::with_defaults();
        let params = make_test_params(5, 1000);
        let validated_blocks = HashSet::new();

        let result = validator
            .validate_block(&params, 4, 1000, &validated_blocks)
            .unwrap();

        assert_eq!(result.block_hash, params.block_hash);
        assert_eq!(result.block_height, 5);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_full_validation_with_warnings() {
        let validator = BlockValidator::with_defaults();
        let params = make_test_params(5, 1005); // Slightly in future
        let validated_blocks = HashSet::new();

        let result = validator
            .validate_block(&params, 4, 1000, &validated_blocks)
            .unwrap();

        assert!(!result.warnings.is_empty());
        assert!(matches!(
            result.warnings[0],
            ValidationWarning::SlightlyFutureTimestamp { .. }
        ));
    }

    #[test]
    fn test_full_validation_duplicate_rejected() {
        let validator = BlockValidator::with_defaults();
        let params = make_test_params(5, 1000);
        let mut validated_blocks = HashSet::new();
        validated_blocks.insert(params.block_hash);

        let result = validator.validate_block(&params, 4, 1000, &validated_blocks);
        assert!(matches!(
            result,
            Err(BlockValidationError::DuplicateBlock { .. })
        ));
    }

    #[test]
    fn test_full_validation_zero_difficulty_rejected() {
        let validator = BlockValidator::with_defaults();
        let mut params = make_test_params(5, 1000);
        params.difficulty = [0u8; 32]; // Zero difficulty
        let validated_blocks = HashSet::new();

        let result = validator.validate_block(&params, 4, 1000, &validated_blocks);
        assert!(matches!(result, Err(BlockValidationError::ZeroDifficulty)));
    }
}
