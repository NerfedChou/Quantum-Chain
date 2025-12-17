//! # Input Validation
//!
//! Validation utilities for adapter inputs.

use crate::domain::errors::StorageError;

/// Maximum allowed block height to prevent overflow attacks.
pub const MAX_BLOCK_HEIGHT: u64 = u64::MAX - 1;

/// Maximum allowed count for batch operations.
pub const MAX_BATCH_COUNT: u64 = 1000;

/// Validates that a block height is within acceptable range.
///
/// # Security
///
/// Prevents potential overflow attacks with extremely large heights.
pub fn validate_block_height(height: u64) -> Result<(), StorageError> {
    if height > MAX_BLOCK_HEIGHT {
        return Err(StorageError::HeightNotFound { height });
    }
    Ok(())
}

/// Validates that a batch count is within limits.
///
/// # Security
///
/// Prevents resource exhaustion attacks via oversized batch requests.
pub fn validate_batch_count(count: u64) -> u64 {
    count.min(MAX_BATCH_COUNT)
}

/// Validates that an API method name is well-formed.
///
/// # Security
///
/// Prevents injection attacks via malformed method names.
pub fn validate_method_name(method: &str) -> bool {
    method
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}
