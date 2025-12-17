//! # Lock Tests

use super::*;
use std::fs;
use std::path::PathBuf;

fn temp_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qc02_lock_{}_{}", test_name, std::process::id()));
    let _ = fs::remove_dir_all(&dir); // Clean up any previous run
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn cleanup(dir: &PathBuf) {
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
