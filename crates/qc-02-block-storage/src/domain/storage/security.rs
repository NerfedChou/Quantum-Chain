//! # Storage Security
//!
//! Security-critical validation for storage operations.
//!
//! ## Security Invariants (SPEC-02 Section 2.6)
//!
//! - INVARIANT-3: Data Integrity (checksum verification)
//! - INVARIANT-5: Finalization Monotonicity
//! - INVARIANT-6: Genesis Immutability

use shared_types::Hash;

/// Security validation errors for storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageSecurityError {
    /// Checksum mismatch detected.
    ChecksumMismatch {
        block_hash: Hash,
        expected: u32,
        actual: u32,
    },
    /// Attempted to modify genesis block.
    GenesisModificationAttempt { existing: Hash, attempted: Hash },
    /// Finalization would regress.
    FinalizationRegression { current: u64, attempted: u64 },
    /// Block size exceeds limit.
    BlockSizeExceeded { size: usize, max: usize },
    /// Hash is all zeros (invalid).
    InvalidHash,
}

impl std::fmt::Display for StorageSecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ChecksumMismatch {
                block_hash,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "checksum mismatch for block {:x?}: expected {}, got {}",
                    &block_hash[..4],
                    expected,
                    actual
                )
            }
            Self::GenesisModificationAttempt { existing, attempted } => {
                write!(
                    f,
                    "attempted to modify genesis from {:x?} to {:x?}",
                    &existing[..4],
                    &attempted[..4]
                )
            }
            Self::FinalizationRegression { current, attempted } => {
                write!(
                    f,
                    "finalization regression from {} to {}",
                    current, attempted
                )
            }
            Self::BlockSizeExceeded { size, max } => {
                write!(f, "block size {} exceeds max {}", size, max)
            }
            Self::InvalidHash => write!(f, "block hash cannot be zero"),
        }
    }
}

/// Security limits for storage operations.
pub mod limits {
    /// Maximum block size in bytes.
    pub const MAX_BLOCK_SIZE: usize = 10 * 1024 * 1024; // 10 MB

    /// Minimum disk space percentage for writes.
    pub const MIN_DISK_SPACE_PERCENT: f32 = 5.0;

    /// Maximum transaction count per block.
    pub const MAX_TRANSACTIONS_PER_BLOCK: usize = 10_000;
}

/// Validate checksum against computed value.
pub fn verify_checksum(
    block_hash: Hash,
    expected: u32,
    actual: u32,
) -> Result<(), StorageSecurityError> {
    if expected != actual {
        return Err(StorageSecurityError::ChecksumMismatch {
            block_hash,
            expected,
            actual,
        });
    }
    Ok(())
}

/// Validate block size against limit.
pub fn verify_block_size(size: usize) -> Result<(), StorageSecurityError> {
    if size > limits::MAX_BLOCK_SIZE {
        return Err(StorageSecurityError::BlockSizeExceeded {
            size,
            max: limits::MAX_BLOCK_SIZE,
        });
    }
    Ok(())
}

/// Validate that a block hash is not all zeros.
pub fn verify_block_hash_nonzero(hash: &Hash) -> Result<(), StorageSecurityError> {
    if hash.iter().all(|&b| b == 0) {
        return Err(StorageSecurityError::InvalidHash);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_checksum_matches() {
        let hash = [0xAB; 32];
        assert!(verify_checksum(hash, 12345, 12345).is_ok());
    }

    #[test]
    fn test_verify_checksum_mismatch() {
        let hash = [0xAB; 32];
        let result = verify_checksum(hash, 12345, 99999);
        assert!(matches!(
            result,
            Err(StorageSecurityError::ChecksumMismatch { .. })
        ));
    }

    #[test]
    fn test_verify_block_size_ok() {
        assert!(verify_block_size(1024 * 1024).is_ok()); // 1 MB
    }

    #[test]
    fn test_verify_block_size_exceeded() {
        let result = verify_block_size(20 * 1024 * 1024); // 20 MB
        assert!(matches!(
            result,
            Err(StorageSecurityError::BlockSizeExceeded { .. })
        ));
    }

    #[test]
    fn test_verify_block_hash_nonzero_ok() {
        let mut hash = [0u8; 32];
        hash[0] = 1;
        assert!(verify_block_hash_nonzero(&hash).is_ok());
    }

    #[test]
    fn test_verify_block_hash_nonzero_fail() {
        let hash = [0u8; 32];
        let result = verify_block_hash_nonzero(&hash);
        assert!(matches!(result, Err(StorageSecurityError::InvalidHash)));
    }
}
