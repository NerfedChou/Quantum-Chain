//! # Assembler Security
//!
//! Security-critical types and validation for the assembly buffer.
//!
//! ## Security Invariants (SPEC-02 Section 2.6)
//!
//! - INVARIANT-7: Assembly Timeout (prevents memory exhaustion)
//! - INVARIANT-8: Bounded Buffer (prevents memory bomb attacks)

use super::config::AssemblyConfig;

/// Security-related constants for the assembler.
pub mod limits {
    /// Minimum timeout (prevents DoS via rapid cleanup).
    pub const MIN_TIMEOUT_SECS: u64 = 5;

    /// Maximum timeout (prevents stale memory buildup).
    pub const MAX_TIMEOUT_SECS: u64 = 300; // 5 minutes

    /// Minimum buffer size.
    pub const MIN_PENDING_ASSEMBLIES: usize = 10;

    /// Maximum buffer size (prevents memory exhaustion).
    pub const MAX_PENDING_ASSEMBLIES: usize = 10_000;

    /// Maximum block size in bytes for assembly validation.
    pub const MAX_BLOCK_SIZE_BYTES: usize = 10 * 1024 * 1024; // 10 MB
}

/// Security validation errors for the assembler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblerSecurityError {
    /// Timeout is below minimum threshold.
    TimeoutTooShort { value: u64, minimum: u64 },
    /// Timeout exceeds maximum threshold.
    TimeoutTooLong { value: u64, maximum: u64 },
    /// Buffer size is below minimum.
    BufferTooSmall { value: usize, minimum: usize },
    /// Buffer size exceeds maximum.
    BufferTooLarge { value: usize, maximum: usize },
}

impl std::fmt::Display for AssemblerSecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TimeoutTooShort { value, minimum } => {
                write!(f, "timeout {}s below minimum {}s", value, minimum)
            }
            Self::TimeoutTooLong { value, maximum } => {
                write!(f, "timeout {}s exceeds maximum {}s", value, maximum)
            }
            Self::BufferTooSmall { value, minimum } => {
                write!(f, "buffer size {} below minimum {}", value, minimum)
            }
            Self::BufferTooLarge { value, maximum } => {
                write!(f, "buffer size {} exceeds maximum {}", value, maximum)
            }
        }
    }
}

/// Validate assembly configuration against security constraints.
///
/// ## Security
///
/// This function enforces the security bounds defined in `limits` module
/// to prevent DoS attacks via configuration manipulation.
pub fn validate_config(config: &AssemblyConfig) -> Result<(), AssemblerSecurityError> {
    // Validate timeout
    if config.assembly_timeout_secs < limits::MIN_TIMEOUT_SECS {
        return Err(AssemblerSecurityError::TimeoutTooShort {
            value: config.assembly_timeout_secs,
            minimum: limits::MIN_TIMEOUT_SECS,
        });
    }

    if config.assembly_timeout_secs > limits::MAX_TIMEOUT_SECS {
        return Err(AssemblerSecurityError::TimeoutTooLong {
            value: config.assembly_timeout_secs,
            maximum: limits::MAX_TIMEOUT_SECS,
        });
    }

    // Validate buffer size
    if config.max_pending_assemblies < limits::MIN_PENDING_ASSEMBLIES {
        return Err(AssemblerSecurityError::BufferTooSmall {
            value: config.max_pending_assemblies,
            minimum: limits::MIN_PENDING_ASSEMBLIES,
        });
    }

    if config.max_pending_assemblies > limits::MAX_PENDING_ASSEMBLIES {
        return Err(AssemblerSecurityError::BufferTooLarge {
            value: config.max_pending_assemblies,
            maximum: limits::MAX_PENDING_ASSEMBLIES,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_config_accepts_defaults() {
        let config = AssemblyConfig::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_rejects_short_timeout() {
        let config = AssemblyConfig::new(1, 1000);
        let result = validate_config(&config);
        assert!(matches!(
            result,
            Err(AssemblerSecurityError::TimeoutTooShort { .. })
        ));
    }

    #[test]
    fn test_validate_config_rejects_large_buffer() {
        let config = AssemblyConfig::new(30, 50_000);
        let result = validate_config(&config);
        assert!(matches!(
            result,
            Err(AssemblerSecurityError::BufferTooLarge { .. })
        ));
    }
}
