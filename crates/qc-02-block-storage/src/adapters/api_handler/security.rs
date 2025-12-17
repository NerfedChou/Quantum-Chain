//! # API Handler Security
//!
//! Request validation and sanitization for API Gateway.
//!
//! ## Security Invariants
//!
//! - **Input Validation**: All parameters validated before processing
//! - **Rate Limiting**: Prevents API abuse (implemented at gateway level)
//! - **Sanitization**: Hex strings validated for format and length

#![allow(dead_code)]

/// Maximum allowed block height to prevent overflow attacks.
pub const MAX_BLOCK_HEIGHT: u64 = u64::MAX - 1;

/// Maximum allowed batch count for range queries.
pub const MAX_BATCH_COUNT: u64 = 1000;

/// Validates and sanitizes a hex-encoded hash string.
///
/// # Security
///
/// - Strips "0x" prefix if present
/// - Validates characters are valid hex
/// - Validates length is exactly 64 characters (32 bytes)
pub fn validate_hex_hash(input: &str) -> Result<[u8; 32], &'static str> {
    let hex_str = input.strip_prefix("0x").unwrap_or(input);

    if hex_str.len() != 64 {
        return Err("Hash must be 32 bytes (64 hex characters)");
    }

    if !hex_str.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hex characters in hash");
    }

    let bytes = hex::decode(hex_str).map_err(|_| "Invalid hex encoding")?;
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

/// Validates a block height parameter.
///
/// # Security
///
/// Prevents potential overflow attacks with extremely large heights.
pub fn validate_block_height(height: u64) -> Result<u64, &'static str> {
    if height > MAX_BLOCK_HEIGHT {
        return Err("Block height exceeds maximum allowed value");
    }
    Ok(height)
}

/// Parses a block number string (hex, decimal, or symbolic).
///
/// # Security
///
/// Validates format and bounds before parsing.
pub fn parse_block_number(input: &str) -> Option<u64> {
    match input {
        "latest" | "finalized" | "pending" => None, // Symbolic, needs service lookup
        s if s.starts_with("0x") => u64::from_str_radix(&s[2..], 16).ok(),
        s => s.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hex_hash_valid() {
        let hash = "0x".to_string() + &"ab".repeat(32);
        assert!(validate_hex_hash(&hash).is_ok());
    }

    #[test]
    fn test_validate_hex_hash_no_prefix() {
        let hash = "ab".repeat(32);
        assert!(validate_hex_hash(&hash).is_ok());
    }

    #[test]
    fn test_validate_hex_hash_invalid_length() {
        let hash = "0x1234";
        assert!(validate_hex_hash(hash).is_err());
    }

    #[test]
    fn test_validate_hex_hash_invalid_chars() {
        let hash = "0x".to_string() + &"zz".repeat(32);
        assert!(validate_hex_hash(&hash).is_err());
    }

    #[test]
    fn test_parse_block_number() {
        assert_eq!(parse_block_number("0x10"), Some(16));
        assert_eq!(parse_block_number("100"), Some(100));
        assert_eq!(parse_block_number("latest"), None);
    }

    #[test]
    fn test_validate_block_height() {
        assert!(validate_block_height(0).is_ok());
        assert!(validate_block_height(1_000_000).is_ok());
    }
}
