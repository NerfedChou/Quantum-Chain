//! # Integrity Module
//!
//! Error types and data integrity checking for the Block Storage subsystem.
//!
//! ## Module Structure
//!
//! - `errors` - Domain error types (StorageError, KVStoreError, FSError)
//! - `security` - Checksum verification and corruption detection
//! - `tests` - Unit tests

mod errors;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export error types
pub use errors::{FSError, KVStoreError, SerializationError, StorageError};
