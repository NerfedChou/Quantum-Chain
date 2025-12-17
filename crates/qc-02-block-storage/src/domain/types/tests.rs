//! # Types Tests

use super::*;

#[test]
fn test_storage_config_default() {
    let config = StorageConfig::default();
    assert_eq!(config.min_disk_space_percent, 5);
    assert_eq!(config.max_block_size, 10 * 1024 * 1024);
    assert!(config.verify_checksums());
}

#[test]
fn test_storage_config_builder() {
    let config = StorageConfig::new()
        .with_min_disk_space(10)
        .with_max_block_size(5 * 1024 * 1024)
        .with_persist_transaction_index(true);

    assert_eq!(config.min_disk_space_percent, 10);
    assert_eq!(config.max_block_size, 5 * 1024 * 1024);
    assert!(config.persist_transaction_index);
}

#[test]
fn test_key_prefix_block() {
    let key = KeyPrefix::block_key(&[0xAB; 32]);
    assert!(key.starts_with(b"b:"));
    assert_eq!(key.len(), 2 + 32);
}

#[test]
fn test_key_prefix_height() {
    let key = KeyPrefix::height_key(1234);
    assert!(key.starts_with(b"h:"));
    assert_eq!(key.len(), 2 + 8);
}

#[test]
fn test_key_prefix_transaction() {
    let tx_hash = [0xCD; 32];
    let key = KeyPrefix::transaction_key(&tx_hash);
    assert!(key.starts_with(b"t:"));
}

#[test]
fn test_key_prefix_metadata() {
    let key = KeyPrefix::metadata_key();
    assert!(key.starts_with(b"m:"));
}

#[test]
fn test_transaction_location_new() {
    let block_hash = [1u8; 32];
    let merkle_root = [2u8; 32];
    let loc = TransactionLocation::new(block_hash, 100, 5, merkle_root);

    assert_eq!(loc.block_hash, block_hash);
    assert_eq!(loc.block_height, 100);
    assert_eq!(loc.transaction_index, 5);
    assert_eq!(loc.merkle_root, merkle_root);
}
