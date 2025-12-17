//! # File Lock Implementation
//!
//! Uses `fs2` for cross-platform file locking (flock on Unix, LockFile on Windows).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;

// =============================================================================
// ERROR TYPES
// =============================================================================

/// Errors from database locking
#[derive(Debug)]
pub enum LockError {
    /// Lock file could not be created
    CreateFailed(io::Error),
    /// Database is already locked by another process
    AlreadyLocked { pid: Option<u32>, path: PathBuf },
    /// Failed to write PID to lock file
    WriteFailed(io::Error),
}

impl std::fmt::Display for LockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockError::CreateFailed(e) => write!(f, "Failed to create lock file: {}", e),
            LockError::AlreadyLocked { pid, path } => {
                if let Some(p) = pid {
                    write!(
                        f,
                        "Database already in use by process {} ({})",
                        p,
                        path.display()
                    )
                } else {
                    write!(f, "Database already in use ({})", path.display())
                }
            }
            LockError::WriteFailed(e) => write!(f, "Failed to write PID to lock file: {}", e),
        }
    }
}

impl std::error::Error for LockError {}

// =============================================================================
// DATABASE LOCK
// =============================================================================

/// Exclusive lock on a database directory.
///
/// Acquired on service startup, released on drop (RAII).
///
/// # Example
///
/// ```ignore
/// let lock = DatabaseLock::acquire(Path::new("/data/blockchain"))?;
/// // Lock is held until `lock` goes out of scope
/// ```
pub struct DatabaseLock {
    /// The lock file handle (kept open to maintain lock)
    file: File,
    /// Path to the lock file
    path: PathBuf,
    /// PID of this process
    pid: u32,
}

impl DatabaseLock {
    /// Lock file name
    const LOCK_FILE: &'static str = "LOCK";

    /// Acquire an exclusive lock on the data directory.
    ///
    /// Retries with exponential backoff up to DEFAULT_LOCK_TIMEOUT.
    /// Detects and cleans up stale locks from crashed processes.
    ///
    /// # Errors
    ///
    /// Returns `LockError::AlreadyLocked` if another process holds the lock
    /// and the timeout expires.
    pub fn acquire(data_dir: &Path) -> Result<Self, LockError> {
        use super::security::{
            is_process_running, validate_lock_path, DEFAULT_LOCK_TIMEOUT, MAX_LOCK_AGE,
        };
        use std::time::{Duration, Instant};

        let deadline = Instant::now() + DEFAULT_LOCK_TIMEOUT;
        let lock_path = data_dir.join(Self::LOCK_FILE);
        let mut retry_delay = Duration::from_millis(50);

        loop {
            // Security: Validate lock path is within data directory
            if lock_path.exists() && !validate_lock_path(data_dir, &lock_path) {
                return Err(LockError::CreateFailed(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Lock path escapes data directory",
                )));
            }

            // Check for stale lock before attempting acquisition
            if Self::is_lock_stale(&lock_path, MAX_LOCK_AGE) {
                let _ = std::fs::remove_file(&lock_path);
            }

            // Create or open lock file
            let file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&lock_path)
                .map_err(LockError::CreateFailed)?;

            // Try to acquire exclusive lock (non-blocking)
            match file.try_lock_exclusive() {
                Ok(()) => {
                    // Success - write our PID to the lock file
                    let pid = std::process::id();
                    let mut locked_file = file;
                    writeln!(locked_file, "{}", pid).map_err(LockError::WriteFailed)?;
                    locked_file.sync_all().map_err(LockError::WriteFailed)?;

                    return Ok(Self {
                        file: locked_file,
                        path: lock_path,
                        pid,
                    });
                }
                Err(_) => {
                    // Try to read existing PID for better error message
                    let existing_pid = Self::read_existing_pid(&lock_path);

                    // Security: Check if the process holding the lock is still running
                    if let Some(pid) = existing_pid {
                        if !is_process_running(pid) {
                            // Stale lock from crashed process - remove and retry immediately
                            drop(file);
                            let _ = std::fs::remove_file(&lock_path);
                            continue;
                        }
                    }

                    // Check timeout
                    if Instant::now() >= deadline {
                        return Err(LockError::AlreadyLocked {
                            pid: existing_pid,
                            path: lock_path,
                        });
                    }

                    // Retry with exponential backoff (capped at 500ms)
                    drop(file);
                    std::thread::sleep(retry_delay);
                    retry_delay = (retry_delay * 2).min(Duration::from_millis(500));
                }
            }
        }
    }

    /// Check if a lock file is stale based on modification time.
    fn is_lock_stale(lock_path: &Path, max_age: std::time::Duration) -> bool {
        lock_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.elapsed().ok())
            .map(|age| age > max_age)
            .unwrap_or(false)
    }

    /// Get the PID of the process holding the lock
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the path to the lock file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read PID from existing lock file (for error messages)
    fn read_existing_pid(path: &Path) -> Option<u32> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }
}

impl Drop for DatabaseLock {
    fn drop(&mut self) {
        // Unlock the file (release flock) - fs2::FileExt::unlock is stable
        #[allow(clippy::incompatible_msrv)]
        let _ = self.file.unlock();
        // Optionally remove lock file (not strictly necessary)
        let _ = std::fs::remove_file(&self.path);
    }
}
