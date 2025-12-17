//! # API Handler Types
//!
//! Data types for API responses.

use serde::{Deserialize, Serialize};

/// QC-02 specific metrics for the admin panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qc02Metrics {
    /// Latest stored block height
    pub latest_height: u64,
    /// Latest finalized block height
    pub finalized_height: u64,
    /// Total number of blocks stored
    pub total_blocks: u64,
    /// Genesis block hash (hex encoded)
    pub genesis_hash: Option<String>,
    /// Storage format version
    pub storage_version: u16,
    /// Disk space used in bytes
    pub disk_used_bytes: u64,
    /// Disk capacity in bytes
    pub disk_capacity_bytes: u64,
    /// Disk usage percentage (0-100)
    pub disk_usage_percent: u8,
    /// Number of pending block assemblies
    pub pending_assemblies: u32,
    /// Assembly timeout in seconds
    pub assembly_timeout_secs: u32,
}

/// Pending assembly info for admin panel display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPendingAssembly {
    /// Block hash (hex encoded)
    pub block_hash: String,
    /// Whether BlockValidated has been received
    pub has_validated_block: bool,
    /// Whether MerkleRootComputed has been received
    pub has_merkle_root: bool,
    /// Whether StateRootComputed has been received
    pub has_state_root: bool,
    /// When the assembly started (Unix timestamp)
    pub started_at: u64,
    /// Time elapsed in seconds
    pub elapsed_secs: u64,
}
