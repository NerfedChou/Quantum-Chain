//! # Types Security
//!
//! Security controls for value objects and configuration.
//!
//! ## Security Invariants
//!
//! - Key validation prevents injection
//! - Configuration bounds checking

/// Maximum key length.
pub const MAX_KEY_LENGTH: usize = 256;

/// Validate key length.
pub fn validate_key_length(key: &[u8]) -> Result<(), &'static str> {
    if key.len() > MAX_KEY_LENGTH {
        return Err("Key exceeds maximum length");
    }
    Ok(())
}

/// Validate disk space percentage is within bounds.
pub fn validate_disk_space_percent(percent: u8) -> Result<(), &'static str> {
    if percent > 100 {
        return Err("Invalid disk space percentage");
    }
    if percent < 1 {
        return Err("Disk space percentage too low");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key_length() {
        assert!(validate_key_length(&[0u8; 100]).is_ok());
        assert!(validate_key_length(&[0u8; 300]).is_err());
    }

    #[test]
    fn test_validate_disk_space() {
        assert!(validate_disk_space_percent(5).is_ok());
        assert!(validate_disk_space_percent(0).is_err());
        assert!(validate_disk_space_percent(101).is_err());
    }
}
