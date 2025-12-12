//! # Block Compression
//!
//! Dictionary-based Zstd compression for block storage.
//!
//! ## Algorithm
//!
//! 1. **Training**: Pre-train a dictionary from sample blocks (offline)
//! 2. **Write Path**: `zstd::compress_with_dict(block_bytes, dictionary)`
//! 3. **Read Path**: `zstd::decompress_with_dict(stored_data, dictionary)`
//!
//! ## Benefits
//!
//! - 30-40% better compression vs standard Zstd for small blockchain data
//! - Extremely fast (Zstd is CPU-efficient)
//! - Reduces disk I/O latency and storage costs

use std::io::{self, Read, Write};

// =============================================================================
// COMPRESSION CONFIGURATION
// =============================================================================

/// Configuration for block compression
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Pre-trained dictionary bytes (100KB typical)
    /// None = use standard compression without dictionary
    pub dictionary: Option<Vec<u8>>,
    /// Compression level (1-22, default 3)
    /// 1-3 = fast, 10-15 = balanced, 19-22 = max compression
    pub level: i32,
    /// Enable compression (can be disabled for debugging)
    pub enabled: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            dictionary: None,
            level: 3, // Fast compression
            enabled: true,
        }
    }
}

impl CompressionConfig {
    /// Create config with dictionary
    pub fn with_dictionary(dict: Vec<u8>) -> Self {
        Self {
            dictionary: Some(dict),
            level: 3,
            enabled: true,
        }
    }

    /// Create config for testing (no dictionary, fast)
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            dictionary: None,
            level: 1,
            enabled: true,
        }
    }
}

// =============================================================================
// COMPRESSION ERROR
// =============================================================================

/// Errors during compression/decompression
#[derive(Debug)]
pub enum CompressionError {
    /// Compression failed
    CompressFailed(io::Error),
    /// Decompression failed
    DecompressFailed(io::Error),
    /// Dictionary is invalid
    InvalidDictionary,
    /// Data appears corrupted
    CorruptedData,
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionError::CompressFailed(e) => write!(f, "Compression failed: {}", e),
            CompressionError::DecompressFailed(e) => write!(f, "Decompression failed: {}", e),
            CompressionError::InvalidDictionary => write!(f, "Invalid compression dictionary"),
            CompressionError::CorruptedData => write!(f, "Compressed data appears corrupted"),
        }
    }
}

impl std::error::Error for CompressionError {}

// =============================================================================
// COMPRESSOR TRAIT
// =============================================================================

/// Trait for block compression implementations
pub trait BlockCompressor: Send + Sync {
    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Check if compression is enabled
    fn is_enabled(&self) -> bool;
}

// =============================================================================
// ZSTD COMPRESSOR
// =============================================================================

/// Zstd-based compressor with optional dictionary support
pub struct ZstdCompressor {
    config: CompressionConfig,
}

impl ZstdCompressor {
    /// Create a new Zstd compressor
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create with default settings (no dictionary, level 3)
    pub fn default_compressor() -> Self {
        Self::new(CompressionConfig::default())
    }
}

impl BlockCompressor for ZstdCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

        // Use dictionary if available
        if let Some(ref dict) = self.config.dictionary {
            let encoder = zstd::dict::EncoderDictionary::copy(dict, self.config.level);
            let mut output = Vec::new();
            let mut encoder =
                zstd::stream::Encoder::with_prepared_dictionary(&mut output, &encoder)
                    .map_err(CompressionError::CompressFailed)?;
            encoder
                .write_all(data)
                .map_err(CompressionError::CompressFailed)?;
            encoder.finish().map_err(CompressionError::CompressFailed)?;
            Ok(output)
        } else {
            // Standard compression without dictionary
            zstd::encode_all(data, self.config.level).map_err(CompressionError::CompressFailed)
        }
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

        // Use dictionary if available
        if let Some(ref dict) = self.config.dictionary {
            let decoder = zstd::dict::DecoderDictionary::copy(dict);
            let mut output = Vec::new();
            let mut decoder = zstd::stream::Decoder::with_prepared_dictionary(data, &decoder)
                .map_err(CompressionError::DecompressFailed)?;
            decoder
                .read_to_end(&mut output)
                .map_err(CompressionError::DecompressFailed)?;
            Ok(output)
        } else {
            // Standard decompression
            zstd::decode_all(data).map_err(CompressionError::DecompressFailed)
        }
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

// =============================================================================
// NO-OP COMPRESSOR (for testing)
// =============================================================================

/// No-op compressor that returns data unchanged
pub struct NoOpCompressor;

impl BlockCompressor for NoOpCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn is_enabled(&self) -> bool {
        false
    }
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zstd_compress_decompress_roundtrip() {
        let compressor = ZstdCompressor::new(CompressionConfig::for_testing());

        let original = b"Hello, blockchain! This is block data that should compress.";
        let compressed = compressor.compress(original).expect("compress");
        let decompressed = compressor.decompress(&compressed).expect("decompress");

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compression_reduces_size() {
        let compressor = ZstdCompressor::new(CompressionConfig::for_testing());

        // Create repetitive data (compresses well)
        let original: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let compressed = compressor.compress(&original).expect("compress");

        // Repetitive data should compress significantly
        assert!(compressed.len() < original.len());
    }

    #[test]
    fn test_disabled_compression_passthrough() {
        let config = CompressionConfig {
            dictionary: None,
            level: 3,
            enabled: false,
        };
        let compressor = ZstdCompressor::new(config);

        let original = b"Test data";
        let result = compressor.compress(original).expect("compress");

        // Should be unchanged when disabled
        assert_eq!(result, original);
    }

    #[test]
    fn test_noop_compressor_passthrough() {
        let compressor = NoOpCompressor;

        let original = b"Test data that should not change";
        let compressed = compressor.compress(original).expect("compress");
        let decompressed = compressor.decompress(&compressed).expect("decompress");

        assert_eq!(compressed, original);
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_dictionary_compression() {
        // Create a simple dictionary from sample data
        let sample_data: Vec<&[u8]> = vec![
            b"block_header: { height: 1, parent: 0x... }",
            b"block_header: { height: 2, parent: 0x... }",
            b"block_header: { height: 3, parent: 0x... }",
        ];

        // Train dictionary (in production, use zstd::dict::from_continuous)
        // For this test, we'll use a simple approach
        let dict: Vec<u8> = sample_data.concat();

        let config = CompressionConfig {
            dictionary: Some(dict.clone()),
            level: 3,
            enabled: true,
        };
        let compressor = ZstdCompressor::new(config);

        let original = b"block_header: { height: 100, parent: 0x... }";
        let compressed = compressor.compress(original).expect("compress");
        let decompressed = compressor.decompress(&compressed).expect("decompress");

        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_empty_data() {
        let compressor = ZstdCompressor::new(CompressionConfig::for_testing());

        let original: &[u8] = b"";
        let compressed = compressor.compress(original).expect("compress empty");
        let decompressed = compressor
            .decompress(&compressed)
            .expect("decompress empty");

        assert_eq!(decompressed, original);
    }
}
