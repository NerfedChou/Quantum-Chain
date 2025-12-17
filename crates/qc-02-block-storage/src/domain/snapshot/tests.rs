//! # Snapshot Tests

use super::*;

#[test]
fn test_snapshot_config_default() {
    let config = SnapshotConfig::default();
    assert!(config.compression);
    assert_eq!(config.format, SnapshotFormat::Single);
    assert_eq!(config.chunk_size, 64 * 1024 * 1024);
}

#[test]
fn test_snapshot_header_new() {
    let header = SnapshotHeader::new(1000, [0xAA; 32], [0xBB; 32], 1001);
    assert_eq!(header.height, 1000);
    assert_eq!(header.version, 1);
    assert_eq!(header.magic, SnapshotHeader::MAGIC);
}

#[test]
fn test_snapshot_header_validate_magic() {
    let mut header = SnapshotHeader::new(100, [0; 32], [0; 32], 101);
    header.magic = [0, 0, 0, 0];

    let result = header.validate();
    assert!(matches!(result, Err(SnapshotError::Corrupted(_))));
}

#[test]
fn test_snapshot_header_validate_version() {
    let mut header = SnapshotHeader::new(100, [0; 32], [0; 32], 101);
    header.version = 999;

    let result = header.validate();
    assert!(matches!(result, Err(SnapshotError::VersionMismatch { .. })));
}

#[test]
fn test_snapshot_header_validate_success() {
    let header = SnapshotHeader::new(100, [0; 32], [0; 32], 101);
    assert!(header.validate().is_ok());
}

#[test]
fn test_snapshot_error_display() {
    let err = SnapshotError::HeightUnavailable(1000);
    assert!(err.to_string().contains("1000"));

    let err = SnapshotError::VersionMismatch {
        expected: 1,
        found: 2,
    };
    assert!(err.to_string().contains("expected 1"));
}
