//! # State Snapshot Export/Import
//!
//! Per SPEC-02 Section 6.1 - enables fast node bootstrapping.
//!
//! ## Features
//!
//! - Export complete chain state to a portable snapshot file
//! - Import snapshot to quickly bootstrap a new node
//! - Optional compression for smaller snapshots

use shared_types::Hash;
use std::path::Path;

// =============================================================================
// SNAPSHOT CONFIGURATION
// =============================================================================

/// Configuration for snapshot operations
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Format for snapshot files
    pub format: SnapshotFormat,
    /// Enable compression in snapshots
    pub compression: bool,
    /// Chunk size for large snapshots (bytes)
    pub chunk_size: usize,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            format: SnapshotFormat::Single,
            compression: true,
            chunk_size: 64 * 1024 * 1024, // 64MB chunks
        }
    }
}

/// Snapshot file format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    /// Single file containing all data
    Single,
    /// Multiple chunk files for large chains
    Chunked,
}

// =============================================================================
// SNAPSHOT INFO
// =============================================================================

/// Information about an exported snapshot
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    /// Path to the snapshot file(s)
    pub path: String,
    /// Height of the snapshot
    pub height: u64,
    /// Block hash at snapshot height
    pub block_hash: Hash,
    /// State root at snapshot height
    pub state_root: Hash,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Number of blocks included
    pub block_count: u64,
    /// Number of transactions included
    pub tx_count: u64,
    /// Whether snapshot is compressed
    pub compressed: bool,
}

// =============================================================================
// SNAPSHOT ERROR
// =============================================================================

/// Errors from snapshot operations
#[derive(Debug)]
pub enum SnapshotError {
    /// I/O error during export/import
    IoError(String),
    /// Snapshot file is corrupted
    Corrupted(String),
    /// Snapshot version mismatch
    VersionMismatch { expected: u32, found: u32 },
    /// Snapshot height unavailable
    HeightUnavailable(u64),
    /// Verification failed
    VerificationFailed(String),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::IoError(msg) => write!(f, "Snapshot I/O error: {}", msg),
            SnapshotError::Corrupted(msg) => write!(f, "Snapshot corrupted: {}", msg),
            SnapshotError::VersionMismatch { expected, found } => {
                write!(
                    f,
                    "Snapshot version mismatch: expected {}, found {}",
                    expected, found
                )
            }
            SnapshotError::HeightUnavailable(h) => {
                write!(f, "Height {} not available for snapshot", h)
            }
            SnapshotError::VerificationFailed(msg) => {
                write!(f, "Snapshot verification failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

// =============================================================================
// SNAPSHOT SERVICE TRAIT
// =============================================================================

/// Trait for types that can export/import snapshots
pub trait SnapshotService {
    /// Export a snapshot at the given height
    fn export_snapshot(
        &self,
        height: u64,
        path: &Path,
        config: &SnapshotConfig,
    ) -> Result<SnapshotInfo, SnapshotError>;

    /// Import a snapshot from the given path
    fn import_snapshot(&mut self, path: &Path) -> Result<SnapshotInfo, SnapshotError>;

    /// Verify a snapshot file without importing
    fn verify_snapshot(&self, path: &Path) -> Result<SnapshotInfo, SnapshotError>;
}

/// Snapshot file header (stored at beginning of file)
#[derive(Debug, Clone)]
pub struct SnapshotHeader {
    /// Magic bytes for identification
    pub magic: [u8; 4],
    /// Version of snapshot format
    pub version: u32,
    /// Height of snapshot
    pub height: u64,
    /// Block hash at height
    pub block_hash: Hash,
    /// State root
    pub state_root: Hash,
    /// Number of blocks
    pub block_count: u64,
    /// Checksum of data section
    pub data_checksum: u32,
}

impl SnapshotHeader {
    /// Magic bytes: "QCSN" (Quantum Chain Snapshot)
    pub const MAGIC: [u8; 4] = [0x51, 0x43, 0x53, 0x4E];
    /// Current version
    pub const VERSION: u32 = 1;

    /// Create a new header
    pub fn new(height: u64, block_hash: Hash, state_root: Hash, block_count: u64) -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            height,
            block_hash,
            state_root,
            block_count,
            data_checksum: 0, // Computed during export
        }
    }

    /// Validate header magic and version
    pub fn validate(&self) -> Result<(), SnapshotError> {
        if self.magic != Self::MAGIC {
            return Err(SnapshotError::Corrupted("Invalid magic bytes".into()));
        }
        if self.version != Self::VERSION {
            return Err(SnapshotError::VersionMismatch {
                expected: Self::VERSION,
                found: self.version,
            });
        }
        Ok(())
    }
}
