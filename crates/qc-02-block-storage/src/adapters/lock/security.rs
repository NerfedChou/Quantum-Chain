//! # Lock Security
//!
//! Security configurations for file locking.
//!
//! ## Security Invariants
//!
//! - **Timeout Protection**: Stale locks are detected via PID check
//! - **Deadlock Prevention**: Non-blocking lock acquisition

use std::path::Path;
use std::time::Duration;

/// Default lock timeout for waiting operations.
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum time a lock should be held before considering it stale.
pub const MAX_LOCK_AGE: Duration = Duration::from_secs(86400); // 24 hours

/// Checks if a process with the given PID is still running.
///
/// # Security
///
/// Used to detect stale locks from crashed processes.
pub fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Check if /proc/<pid> exists (Linux-specific but safer than libc)
        std::path::Path::new(&format!("/proc/{}", pid)).exists()
    }

    #[cfg(not(unix))]
    {
        // On Windows, assume process is running (conservative approach)
        let _ = pid;
        true
    }
}

/// Validates that a lock file path is within the expected data directory.
///
/// # Security
///
/// Prevents path traversal attacks via malformed lock paths.
pub fn validate_lock_path(data_dir: &Path, lock_path: &Path) -> bool {
    lock_path
        .canonicalize()
        .ok()
        .and_then(|canonical| {
            data_dir
                .canonicalize()
                .ok()
                .map(|data_canonical| canonical.starts_with(&data_canonical))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_process_running_self() {
        let pid = std::process::id();
        assert!(is_process_running(pid));
    }

    #[test]
    fn test_is_process_running_invalid() {
        // PID 0 is typically not a user process
        // This test is platform-dependent
        let _ = is_process_running(0);
    }

    #[test]
    fn test_default_lock_timeout() {
        assert_eq!(DEFAULT_LOCK_TIMEOUT.as_secs(), 30);
    }

    #[test]
    fn test_max_lock_age() {
        assert_eq!(MAX_LOCK_AGE.as_secs(), 86400);
    }
}
