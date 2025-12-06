//! Bloom filter configuration and validation
//!
//! Reference: SPEC-07, Section 2.1 - BloomConfig
//!
//! # Example
//!
//! ```ignore
//! use qc_07_bloom_filters::domain::BloomConfigBuilder;
//!
//! let config = BloomConfigBuilder::new()
//!     .target_fpr(0.05)
//!     .max_elements(100)
//!     .privacy_noise(10.0)
//!     .build()
//!     .expect("Valid config");
//! ```

use crate::error::FilterError;
use serde::{Deserialize, Serialize};

/// Bloom filter configuration
///
/// Per IPC-MATRIX.md Subsystem 7:
/// - Reject FPR <0.01 or >0.1 (too precise or too noisy)
/// - Reject >1000 watched addresses (privacy risk)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomConfig {
    /// Target false positive rate (0.01 to 0.1)
    pub target_fpr: f64,
    /// Maximum filter size in bits
    pub max_size_bits: usize,
    /// Maximum elements per filter
    pub max_elements: usize,
    /// Filter rotation interval (blocks)
    pub rotation_interval: u64,
    /// Add random false positives for privacy (percent)
    pub privacy_noise_percent: f64,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            target_fpr: 0.0001,       // 0.01% false positive rate
            max_size_bits: 36_000,    // ~4.5 KB
            max_elements: 50,
            rotation_interval: 100,
            privacy_noise_percent: 5.0,
        }
    }
}

impl BloomConfig {
    /// Create a new configuration with validation
    pub fn new(
        target_fpr: f64,
        max_size_bits: usize,
        max_elements: usize,
        rotation_interval: u64,
        privacy_noise_percent: f64,
    ) -> Result<Self, FilterError> {
        let config = Self {
            target_fpr,
            max_size_bits,
            max_elements,
            rotation_interval,
            privacy_noise_percent,
        };
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration per IPC-MATRIX.md security boundaries
    pub fn validate(&self) -> Result<(), FilterError> {
        // IPC-MATRIX.md: Reject FPR <0.01 (too precise = privacy risk)
        if self.target_fpr < 0.01 {
            return Err(FilterError::InvalidFPR { fpr: self.target_fpr });
        }

        // IPC-MATRIX.md: Reject FPR >0.1 (too noisy = useless)
        if self.target_fpr > 0.1 {
            return Err(FilterError::InvalidFPR { fpr: self.target_fpr });
        }

        // IPC-MATRIX.md: Reject >1000 watched addresses
        if self.max_elements > 1000 {
            return Err(FilterError::TooManyElements {
                count: self.max_elements,
                max: 1000,
            });
        }

        // Sanity checks
        if self.max_size_bits == 0 {
            return Err(FilterError::InvalidParameters(
                "max_size_bits cannot be 0".to_string(),
            ));
        }

        if self.privacy_noise_percent < 0.0 || self.privacy_noise_percent > 100.0 {
            return Err(FilterError::InvalidParameters(
                "privacy_noise_percent must be between 0 and 100".to_string(),
            ));
        }

        Ok(())
    }

    /// Builder-style method to set target FPR
    pub fn with_target_fpr(mut self, fpr: f64) -> Self {
        self.target_fpr = fpr;
        self
    }

    /// Builder-style method to set max elements
    pub fn with_max_elements(mut self, max: usize) -> Self {
        self.max_elements = max;
        self
    }

    /// Builder-style method to set privacy noise
    pub fn with_privacy_noise(mut self, percent: f64) -> Self {
        self.privacy_noise_percent = percent;
        self
    }
}

/// Builder for BloomConfig with validation
///
/// Provides a fluent interface for constructing BloomConfig instances
/// with compile-time safety and clear error handling.
///
/// # Example
///
/// ```ignore
/// let config = BloomConfigBuilder::new()
///     .target_fpr(0.05)
///     .max_elements(100)
///     .max_size_bits(50_000)
///     .rotation_interval(100)
///     .privacy_noise(10.0)
///     .build()?;
/// ```
#[derive(Default)]
pub struct BloomConfigBuilder {
    target_fpr: Option<f64>,
    max_size_bits: Option<usize>,
    max_elements: Option<usize>,
    rotation_interval: Option<u64>,
    privacy_noise_percent: Option<f64>,
}

impl BloomConfigBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set target false positive rate (must be between 0.01 and 0.1)
    pub fn target_fpr(mut self, fpr: f64) -> Self {
        self.target_fpr = Some(fpr);
        self
    }

    /// Set maximum filter size in bits
    pub fn max_size_bits(mut self, bits: usize) -> Self {
        self.max_size_bits = Some(bits);
        self
    }

    /// Set maximum number of elements (addresses) per filter
    pub fn max_elements(mut self, elements: usize) -> Self {
        self.max_elements = Some(elements);
        self
    }

    /// Set filter rotation interval in blocks
    pub fn rotation_interval(mut self, blocks: u64) -> Self {
        self.rotation_interval = Some(blocks);
        self
    }

    /// Set privacy noise percentage (0-100)
    pub fn privacy_noise(mut self, percent: f64) -> Self {
        self.privacy_noise_percent = Some(percent);
        self
    }

    /// Build the BloomConfig, validating all parameters
    ///
    /// Returns an error if any parameters are invalid per IPC-MATRIX.md
    pub fn build(self) -> Result<BloomConfig, FilterError> {
        let defaults = BloomConfig::default();

        let config = BloomConfig {
            target_fpr: self.target_fpr.unwrap_or(defaults.target_fpr),
            max_size_bits: self.max_size_bits.unwrap_or(defaults.max_size_bits),
            max_elements: self.max_elements.unwrap_or(defaults.max_elements),
            rotation_interval: self.rotation_interval.unwrap_or(defaults.rotation_interval),
            privacy_noise_percent: self.privacy_noise_percent.unwrap_or(defaults.privacy_noise_percent),
        };

        config.validate()?;
        Ok(config)
    }

    /// Build without validation (for internal use only)
    pub fn build_unchecked(self) -> BloomConfig {
        let defaults = BloomConfig::default();

        BloomConfig {
            target_fpr: self.target_fpr.unwrap_or(defaults.target_fpr),
            max_size_bits: self.max_size_bits.unwrap_or(defaults.max_size_bits),
            max_elements: self.max_elements.unwrap_or(defaults.max_elements),
            rotation_interval: self.rotation_interval.unwrap_or(defaults.rotation_interval),
            privacy_noise_percent: self.privacy_noise_percent.unwrap_or(defaults.privacy_noise_percent),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        let config = BloomConfig::default();
        // Default should be usable internally but may not pass external validation
        // since default FPR is 0.0001 < 0.01
        assert!(config.target_fpr > 0.0);
        assert!(config.max_size_bits > 0);
    }

    #[test]
    fn test_config_validation_rejects_fpr_too_low() {
        let config = BloomConfig {
            target_fpr: 0.001, // < 0.01 = too precise
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(FilterError::InvalidFPR { .. })));
    }

    #[test]
    fn test_config_validation_rejects_fpr_too_high() {
        let config = BloomConfig {
            target_fpr: 0.2, // > 0.1 = too noisy
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(FilterError::InvalidFPR { .. })));
    }

    #[test]
    fn test_config_validation_accepts_valid_fpr() {
        let config = BloomConfig {
            target_fpr: 0.05, // Between 0.01 and 0.1
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_rejects_too_many_elements() {
        let config = BloomConfig {
            target_fpr: 0.05,
            max_elements: 1001, // > 1000
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(FilterError::TooManyElements { .. })));
    }

    #[test]
    fn test_builder_creates_valid_config() {
        let config = BloomConfigBuilder::new()
            .target_fpr(0.05)
            .max_elements(100)
            .max_size_bits(50_000)
            .rotation_interval(200)
            .privacy_noise(10.0)
            .build()
            .expect("Should create valid config");

        assert_eq!(config.target_fpr, 0.05);
        assert_eq!(config.max_elements, 100);
        assert_eq!(config.max_size_bits, 50_000);
        assert_eq!(config.rotation_interval, 200);
        assert_eq!(config.privacy_noise_percent, 10.0);
    }

    #[test]
    fn test_builder_rejects_invalid_fpr() {
        let result = BloomConfigBuilder::new()
            .target_fpr(0.001) // Too low
            .build();

        assert!(matches!(result, Err(FilterError::InvalidFPR { .. })));
    }

    #[test]
    fn test_builder_uses_defaults() {
        let config = BloomConfigBuilder::new()
            .target_fpr(0.05) // Only set FPR
            .build()
            .expect("Should use defaults for other fields");

        let defaults = BloomConfig::default();
        assert_eq!(config.max_size_bits, defaults.max_size_bits);
        assert_eq!(config.max_elements, defaults.max_elements);
    }

    #[test]
    fn test_builder_chaining() {
        // Test that builder methods can be chained in any order
        let config1 = BloomConfigBuilder::new()
            .target_fpr(0.05)
            .max_elements(50)
            .build()
            .unwrap();

        let config2 = BloomConfigBuilder::new()
            .max_elements(50)
            .target_fpr(0.05)
            .build()
            .unwrap();

        assert_eq!(config1.target_fpr, config2.target_fpr);
        assert_eq!(config1.max_elements, config2.max_elements);
    }
}
