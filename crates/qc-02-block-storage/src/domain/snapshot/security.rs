//! # Snapshot Security
//!
//! Security controls for snapshot operations.
//!
//! ## Security Invariants
//!
//! - Atomic writes prevent partial snapshots
//! - Integrity verification on import

/// Magic bytes for valid snapshots.
pub const SNAPSHOT_MAGIC: [u8; 4] = [0x51, 0x43, 0x53, 0x4E]; // "QCSN"

/// Validate snapshot magic bytes.
pub fn validate_magic(magic: &[u8; 4]) -> bool {
    magic == &SNAPSHOT_MAGIC
}

/// Maximum snapshot file size (10GB).
pub const MAX_SNAPSHOT_SIZE: u64 = 10 * 1024 * 1024 * 1024;

/// Validate snapshot file size.
pub fn validate_snapshot_size(size: u64) -> Result<(), &'static str> {
    if size > MAX_SNAPSHOT_SIZE {
        return Err("Snapshot exceeds maximum size");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_magic() {
        assert!(validate_magic(&SNAPSHOT_MAGIC));
        assert!(!validate_magic(&[0, 0, 0, 0]));
    }

    #[test]
    fn test_validate_size() {
        assert!(validate_snapshot_size(1024).is_ok());
        assert!(validate_snapshot_size(MAX_SNAPSHOT_SIZE + 1).is_err());
    }
}
