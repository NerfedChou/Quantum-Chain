//! # Metrics Security
//!
//! Security controls for metrics operations.
//!
//! ## Security Invariants
//!
//! - Overflow protection for counters
//! - Sanitization of exported metrics

/// Maximum counter value before warning (prevents overflow).
pub const MAX_COUNTER_VALUE: u64 = u64::MAX - 1_000_000;

/// Validate counter value is within safe bounds.
pub fn validate_counter(value: u64) -> bool {
    value < MAX_COUNTER_VALUE
}

/// Sanitize metric name for safe export.
pub fn sanitize_metric_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_counter_normal() {
        assert!(validate_counter(1000));
        assert!(validate_counter(1_000_000_000));
    }

    #[test]
    fn test_validate_counter_near_max() {
        assert!(!validate_counter(u64::MAX));
    }

    #[test]
    fn test_sanitize_metric_name() {
        assert_eq!(sanitize_metric_name("valid_name"), "valid_name");
        assert_eq!(sanitize_metric_name("bad<script>"), "badscript");
    }
}
