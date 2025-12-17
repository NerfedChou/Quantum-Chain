//! # Integrity Security
//!
//! Security-critical validation for data integrity.
//!
//! ## Security Invariants
//!
//! - INVARIANT-3: Data checksums are always verified on read
//! - Corruption detection must be deterministic and reproducible
//! - Non-canonical encoding detection prevents hash malleability

use super::StorageError;
use shared_types::Hash;

/// Maximum block size (10MB) - prevents memory exhaustion attacks.
pub const MAX_BLOCK_SIZE: usize = 10 * 1024 * 1024;

/// Minimum checksum value (for validation tests).
pub const MIN_VALID_CHECKSUM: u32 = 1;

/// Validate that a checksum is non-zero.
///
/// Zero checksums indicate uninitialized or corrupted data.
pub fn validate_checksum(checksum: u32) -> Result<(), StorageError> {
    if checksum == 0 {
        return Err(StorageError::DataCorruption {
            block_hash: [0u8; 32],
            expected_checksum: MIN_VALID_CHECKSUM,
            actual_checksum: 0,
        });
    }
    Ok(())
}

/// Create a data corruption error for a specific block.
pub fn corruption_error(block_hash: Hash, expected: u32, actual: u32) -> StorageError {
    StorageError::DataCorruption {
        block_hash,
        expected_checksum: expected,
        actual_checksum: actual,
    }
}

/// Validate block size is within limits.
pub fn validate_block_size(size: usize, max_size: usize) -> Result<(), StorageError> {
    if size > max_size {
        return Err(StorageError::BlockTooLarge { size, max_size });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_checksum_rejects_zero() {
        let result = validate_checksum(0);
        assert!(matches!(result, Err(StorageError::DataCorruption { .. })));
    }

    #[test]
    fn test_validate_checksum_accepts_nonzero() {
        assert!(validate_checksum(1).is_ok());
        assert!(validate_checksum(u32::MAX).is_ok());
    }

    #[test]
    fn test_validate_block_size() {
        assert!(validate_block_size(100, 1000).is_ok());
        assert!(validate_block_size(1001, 1000).is_err());
    }
}
