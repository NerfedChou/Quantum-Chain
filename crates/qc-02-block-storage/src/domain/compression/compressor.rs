//! # Block Compression
//!
//! Dictionary-based Zstd compression for block storage.

use std::io::{self, Read, Write};

// =============================================================================
// COMPRESSION CONFIGURATION
// =============================================================================

/// Configuration for block compression
#[derive(Debug, Clone)]
pub struct CompressionConfig {
    /// Pre-trained dictionary bytes (100KB typical)
    pub dictionary: Option<Vec<u8>>,
    /// Compression level (1-22, default 3)
    pub level: i32,
    /// Enable compression
    pub enabled: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            dictionary: None,
            level: 3,
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

    /// Create with default settings
    pub fn default_compressor() -> Self {
        Self::new(CompressionConfig::default())
    }
}

impl BlockCompressor for ZstdCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

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
            zstd::encode_all(data, self.config.level).map_err(CompressionError::CompressFailed)
        }
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        if !self.config.enabled {
            return Ok(data.to_vec());
        }

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
            zstd::decode_all(data).map_err(CompressionError::DecompressFailed)
        }
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }
}

// =============================================================================
// NO-OP COMPRESSOR
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
