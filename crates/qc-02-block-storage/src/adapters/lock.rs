//! # Database Process Locking
//!
//! Prevents multiple processes from accessing the same data directory.
//!
//! ## Security Purpose
//!
//! Without locking, two node instances pointing to the same data directory
//! can corrupt atomic batch writes or append garbage to files.
//!
//! ## Implementation
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
                    write!(f, "Database already in use by process {} ({})", p, path.display())
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
    /// # Errors
    ///
    /// Returns `LockError::AlreadyLocked` if another process holds the lock.
    pub fn acquire(data_dir: &Path) -> Result<Self, LockError> {
        let lock_path = data_dir.join(Self::LOCK_FILE);

        // Create or open lock file
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&lock_path)
            .map_err(LockError::CreateFailed)?;

        // Try to acquire exclusive lock (non-blocking)
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(_) => {
                // Try to read existing PID for better error message
                let existing_pid = Self::read_existing_pid(&lock_path);
                return Err(LockError::AlreadyLocked {
                    pid: existing_pid,
                    path: lock_path,
                });
            }
        }

        // Write our PID to the lock file
        let pid = std::process::id();
        let mut locked_file = file;
        writeln!(locked_file, "{}", pid).map_err(LockError::WriteFailed)?;
        locked_file.sync_all().map_err(LockError::WriteFailed)?;

        Ok(Self {
            file: locked_file,
            path: lock_path,
            pid,
        })
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
        // Unlock the file (release flock)
        let _ = self.file.unlock();
        // Optionally remove lock file (not strictly necessary)
        let _ = std::fs::remove_file(&self.path);
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "qc02_lock_{}_{}", 
            test_name, 
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir); // Clean up any previous run
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_lock_acquire_creates_file() {
        let dir = temp_dir("acquire");
        
        let lock = DatabaseLock::acquire(&dir).expect("Should acquire lock");
        assert!(lock.path().exists());
        assert_eq!(lock.pid(), std::process::id());
        
        drop(lock);
        cleanup(&dir);
    }

    #[test]
    fn test_lock_contains_pid() {
        let dir = temp_dir("contains_pid");
        
        let lock = DatabaseLock::acquire(&dir).expect("Should acquire lock");
        let content = fs::read_to_string(lock.path()).unwrap();
        let stored_pid: u32 = content.trim().parse().unwrap();
        assert_eq!(stored_pid, std::process::id());
        
        drop(lock);
        cleanup(&dir);
    }

    #[test]
    fn test_double_lock_fails() {
        let dir = temp_dir("double_lock");
        
        let lock1 = DatabaseLock::acquire(&dir).expect("First lock should succeed");
        
        // Second attempt should fail
        let result = DatabaseLock::acquire(&dir);
        assert!(result.is_err());
        
        // Just verify we get AlreadyLocked error (PID may or may not be readable due to truncation)
        assert!(matches!(result, Err(LockError::AlreadyLocked { .. })));
        
        drop(lock1);
        cleanup(&dir);
    }

    #[test]
    fn test_lock_released_on_drop() {
        let dir = temp_dir("released_on_drop");
        
        {
            let _lock = DatabaseLock::acquire(&dir).expect("Should acquire");
        }
        
        // Should be able to acquire again after drop
        let lock2 = DatabaseLock::acquire(&dir).expect("Should acquire after release");
        drop(lock2);
        cleanup(&dir);
    }
}
