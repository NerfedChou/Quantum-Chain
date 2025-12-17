//! # Compression Module
//!
//! Dictionary-based Zstd compression for block storage.

mod compressor;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public types
pub use compressor::{
    BlockCompressor, CompressionConfig, CompressionError, NoOpCompressor, ZstdCompressor,
};
