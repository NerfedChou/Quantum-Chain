//! # Repair Security
//!
//! Security controls for repair operations.
//!
//! ## Security Invariants
//!
//! - Repair requires explicit authorization
//! - All repair operations must be logged

/// Repair requires elevated privileges flag.
pub const REPAIR_REQUIRES_AUTHORIZATION: bool = true;

/// Log level for repair operations.
pub const REPAIR_LOG_LEVEL: &str = "WARN";

/// Validate repair authorization.
pub fn validate_repair_authorization(authorized: bool) -> Result<(), &'static str> {
    if REPAIR_REQUIRES_AUTHORIZATION && !authorized {
        return Err("Repair requires authorization");
    }
    Ok(())
}

/// Generate audit log entry for repair operation.
pub fn audit_log_repair(operation: &str, height: u64, success: bool) -> String {
    format!(
        "[REPAIR AUDIT] op={} height={} success={}",
        operation, height, success
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unauthorized_rejected() {
        assert!(validate_repair_authorization(false).is_err());
    }

    #[test]
    fn test_authorized_accepted() {
        assert!(validate_repair_authorization(true).is_ok());
    }

    #[test]
    fn test_audit_log() {
        let log = audit_log_repair("rebuild_index", 1000, true);
        assert!(log.contains("REPAIR AUDIT"));
        assert!(log.contains("1000"));
    }
}
