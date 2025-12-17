//! # Database Process Locking
//!
//! Prevents multiple processes from accessing the same data directory.
//!
//! ## Modules
//!
//! - `flock`: FileLock implementation using fs2
//! - `security`: Lock timeout and deadlock prevention

mod flock;
mod security;
#[cfg(test)]
mod tests;

pub use flock::{DatabaseLock, LockError};
