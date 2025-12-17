//! # Compression Security
//!
//! Security controls for compression operations.
//!
//! ## Security Invariants
//!
//! - Decompression bomb prevention (max output size)
//! - Dictionary validation

/// Maximum decompressed size to prevent decompression bombs (100MB).
pub const MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;

/// Maximum compression ratio allowed (prevents decompression bombs).
pub const MAX_COMPRESSION_RATIO: usize = 100;

/// Validate that decompressed data doesn't exceed limits.
///
/// # Security
///
/// Prevents decompression bomb attacks by limiting output size.
pub fn validate_decompressed_size(
    compressed_size: usize,
    decompressed_size: usize,
) -> Result<(), &'static str> {
    if decompressed_size > MAX_DECOMPRESSED_SIZE {
        return Err("Decompressed size exceeds maximum");
    }

    if compressed_size > 0 {
        let ratio = decompressed_size / compressed_size;
        if ratio > MAX_COMPRESSION_RATIO {
            return Err("Suspicious compression ratio detected");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_normal_size() {
        assert!(validate_decompressed_size(1000, 2000).is_ok());
    }

    #[test]
    fn test_validate_rejects_bomb() {
        // 1KB compressed to 200MB = 200,000x ratio
        assert!(validate_decompressed_size(1024, 200 * 1024 * 1024).is_err());
    }

    #[test]
    fn test_validate_rejects_too_large() {
        assert!(validate_decompressed_size(50 * 1024 * 1024, 150 * 1024 * 1024).is_err());
    }
}
