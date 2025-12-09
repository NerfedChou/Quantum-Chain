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
        self.tx_index.insert(tx_hash, TransactionLocationRepair {
            block_hash,
            tx_index: index,
        });
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

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repair_report_new() {
        let report = RepairReport::new();
        assert_eq!(report.blocks_recovered, 0);
        assert_eq!(report.transactions_indexed, 0);
        assert!(report.lowest_height.is_none());
        assert!(report.highest_height.is_none());
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_repair_report_add_block() {
        let mut report = RepairReport::new();
        
        report.add_block(100, 5);
        assert_eq!(report.blocks_recovered, 1);
        assert_eq!(report.transactions_indexed, 5);
        assert_eq!(report.lowest_height, Some(100));
        assert_eq!(report.highest_height, Some(100));
        
        report.add_block(50, 3);
        assert_eq!(report.blocks_recovered, 2);
        assert_eq!(report.transactions_indexed, 8);
        assert_eq!(report.lowest_height, Some(50));
        assert_eq!(report.highest_height, Some(100));
        
        report.add_block(200, 10);
        assert_eq!(report.blocks_recovered, 3);
        assert_eq!(report.lowest_height, Some(50));
        assert_eq!(report.highest_height, Some(200));
    }

    #[test]
    fn test_repair_report_is_successful() {
        let mut report = RepairReport::new();
        assert!(report.is_successful()); // Empty is OK
        
        report.add_block(1, 0);
        assert!(report.is_successful());
        
        report.add_error(RepairError::new(vec![1, 2, 3], "test error"));
        assert!(report.is_successful()); // Has blocks, so still successful
    }

    #[test]
    fn test_repair_context_record_block() {
        let mut ctx = RepairContext::new();
        
        ctx.record_block(1, [0xAA; 32], 5);
        ctx.record_block(2, [0xBB; 32], 3);
        
        assert_eq!(ctx.block_index.len(), 2);
        assert_eq!(ctx.block_index.get(&1), Some(&[0xAA; 32]));
        assert_eq!(ctx.block_index.get(&2), Some(&[0xBB; 32]));
        assert_eq!(ctx.report.blocks_recovered, 2);
    }

    #[test]
    fn test_repair_context_record_transaction() {
        let mut ctx = RepairContext::new();
        let tx_hash = [0x11; 32];
        let block_hash = [0xAA; 32];
        
        ctx.record_transaction(tx_hash, block_hash, 0);
        
        assert!(ctx.tx_index.contains_key(&tx_hash));
        let loc = ctx.tx_index.get(&tx_hash).unwrap();
        assert_eq!(loc.block_hash, block_hash);
        assert_eq!(loc.tx_index, 0);
    }

    #[test]
    fn test_repair_error_display() {
        let err = RepairFatalError::StorageInaccessible("disk full".to_string());
        assert!(err.to_string().contains("disk full"));
        
        let err = RepairFatalError::EmptyStorage;
        assert!(err.to_string().contains("empty"));
    }
}
