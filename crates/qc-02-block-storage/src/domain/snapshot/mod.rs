//! # Snapshot Module
//!
//! State snapshot export/import per SPEC-02 Section 6.1.

mod header;
pub mod security;

#[cfg(test)]
mod tests;

// Re-export public types
pub use header::{
    SnapshotConfig, SnapshotError, SnapshotFormat, SnapshotHeader, SnapshotInfo, SnapshotService,
};
