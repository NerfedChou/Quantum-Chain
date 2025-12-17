//! # Compression Tests

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

    let original: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
    let compressed = compressor.compress(&original).expect("compress");

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
    let sample_data: Vec<&[u8]> = vec![
        b"block_header: { height: 1, parent: 0x... }",
        b"block_header: { height: 2, parent: 0x... }",
        b"block_header: { height: 3, parent: 0x... }",
    ];

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
