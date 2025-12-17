//! # Self-Healing Index (Disaster Recovery)
//!
//! Provides index repair capability for recovery from database corruption.
//!
//! ## Security Purpose
//!
//! If the KV store index gets corrupted (e.g., process killed during compaction),
//! but raw block data is intact, this module can rebuild the index from scratch.
//!
//! ## Algorithm
//!
//! 1. Scan all block values in KV store
//! 2. For each entry: Parse header, extract hash and height
//! 3. Re-insert index entries (height -> hash mappings)
//! 4. Rebuild transaction index
//! 5. Return report (blocks recovered, errors encountered)

use shared_types::Hash;
use std::collections::HashMap;

// =============================================================================
// REPAIR REPORT
// =============================================================================

/// Result of an index repair operation
#[derive(Debug, Clone, Default)]
pub struct RepairReport {
    /// Number of blocks successfully recovered
    pub blocks_recovered: u64,
    /// Number of transactions indexed
    pub transactions_indexed: u64,
    /// Height of lowest block found
    pub lowest_height: Option<u64>,
    /// Height of highest block found  
    pub highest_height: Option<u64>,
    /// Errors encountered during repair (non-fatal)
    pub errors: Vec<RepairError>,
    /// Duration of repair in milliseconds
    pub duration_ms: u64,
}

impl RepairReport {
    /// Create empty report
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if repair was successful (no fatal errors)
    pub fn is_successful(&self) -> bool {
        self.blocks_recovered > 0 || self.errors.is_empty()
    }

    /// Add a recovered block to the report
    pub fn add_block(&mut self, height: u64, tx_count: u64) {
        self.blocks_recovered += 1;
        self.transactions_indexed += tx_count;

        match self.lowest_height {
            None => self.lowest_height = Some(height),
            Some(h) if height < h => self.lowest_height = Some(height),
            _ => {}
        }

        match self.highest_height {
            None => self.highest_height = Some(height),
            Some(h) if height > h => self.highest_height = Some(height),
            _ => {}
        }
    }

    /// Add an error to the report
    pub fn add_error(&mut self, error: RepairError) {
        self.errors.push(error);
    }
}

/// Errors encountered during repair (non-fatal, logged and continued)
#[derive(Debug, Clone)]
pub struct RepairError {
    /// Key that caused the error
    pub key: Vec<u8>,
    /// Description of the error
    pub message: String,
}

impl RepairError {
    pub fn new(key: Vec<u8>, message: impl Into<String>) -> Self {
        Self {
            key,
            message: message.into(),
        }
    }
}

// =============================================================================
// REPAIR CONTEXT
// =============================================================================

/// Context for holding repair state
#[derive(Debug, Default)]
pub struct RepairContext {
    /// Rebuilt block index (height -> hash)
    pub block_index: HashMap<u64, Hash>,
    /// Rebuilt transaction index (tx_hash -> location)  
    pub tx_index: HashMap<Hash, TransactionLocationRepair>,
    /// Report of repair progress
    pub report: RepairReport,
}

impl RepairContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successfully parsed block
    pub fn record_block(&mut self, height: u64, hash: Hash, tx_count: u64) {
        self.block_index.insert(height, hash);
        self.report.add_block(height, tx_count);
    }

    /// Record a transaction location
    pub fn record_transaction(&mut self, tx_hash: Hash, block_hash: Hash, index: u32) {
        self.tx_index.insert(
            tx_hash,
            TransactionLocationRepair {
                block_hash,
                tx_index: index,
            },
        );
    }
}

/// Transaction location for repair (simplified)
#[derive(Debug, Clone)]
pub struct TransactionLocationRepair {
    pub block_hash: Hash,
    pub tx_index: u32,
}

// =============================================================================
// REPAIR TRAIT
// =============================================================================

/// Trait for types that can be repaired
pub trait Repairable {
    /// Scan stored data and rebuild indexes
    fn repair_index(&mut self) -> Result<RepairReport, RepairFatalError>;
}

/// Fatal errors that prevent repair from completing
#[derive(Debug)]
pub enum RepairFatalError {
    /// Cannot access storage
    StorageInaccessible(String),
    /// Storage is empty (nothing to repair)
    EmptyStorage,
    /// Critical data corruption (cannot proceed)
    CriticalCorruption(String),
}

impl std::fmt::Display for RepairFatalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepairFatalError::StorageInaccessible(msg) => {
                write!(f, "Storage inaccessible: {}", msg)
            }
            RepairFatalError::EmptyStorage => {
                write!(f, "Storage is empty, nothing to repair")
            }
            RepairFatalError::CriticalCorruption(msg) => {
                write!(f, "Critical corruption: {}", msg)
            }
        }
    }
}

impl std::error::Error for RepairFatalError {}
