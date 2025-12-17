//! # Storage Entities
//!
//! Core storage entities for the Block Storage subsystem.
//!
//! ## SPEC-02 Reference
//!
//! - Section 2.2: StoredBlock, BlockIndex, StorageMetadata
//! - Section 2.3: Index Structures
//!
//! ## Module Structure
//!
//! - `block` - StoredBlock struct
//! - `index` - BlockIndex and BlockIndexEntry
//! - `metadata` - StorageMetadata
//! - `security` - Security validation and limits

mod block;
mod index;
mod metadata;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public API
pub use block::StoredBlock;
pub use index::{BlockIndex, BlockIndexEntry};
pub use metadata::StorageMetadata;

/// Unix timestamp in seconds since epoch.
pub type Timestamp = u64;
