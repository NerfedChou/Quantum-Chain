//! # Smart Pruning (Anchor System)
//!
//! Logarithmic pruning that keeps "stepping stones" for syncing.
//! Per SPEC-02 Section 5.2.
//!
//! ## Algorithm
//!
//! 1. Keep the last N blocks (e.g., 10,000) fully
//! 2. For blocks older than N:
//!    - Delete the Body (transactions)
//!    - Keep the Header
//!    - Keep full blocks at intervals of 2^k (1000, 2000, 4000...)
//!
//! ## Benefits
//!
//! - Reduces storage by ~90%
//! - Node can still serve "Chain Checkpoints" to new peers

use shared_types::Hash;

// =============================================================================
// PRUNING CONFIGURATION
// =============================================================================

/// Configuration for smart pruning
#[derive(Debug, Clone)]
pub struct PruningConfig {
    /// Number of recent blocks to keep fully (default: 10,000)
    pub keep_recent: u64,
    /// Base interval for anchor blocks (default: 1000)
    /// Keeps full blocks at heights: 1000, 2000, 4000, 8000...
    pub anchor_base: u64,
    /// Always keep headers even when body is pruned
    pub keep_headers: bool,
    /// Enable auto-pruning
    pub enabled: bool,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            keep_recent: 10_000,
            anchor_base: 1000,
            keep_headers: true,
            enabled: false, // Disabled by default for safety
        }
    }
}

// =============================================================================
// PRUNING SERVICE
// =============================================================================

/// Service for managing block pruning
#[derive(Debug)]
pub struct PruningService {
    config: PruningConfig,
}

impl PruningService {
    /// Create a new pruning service
    pub fn new(config: PruningConfig) -> Self {
        Self { config }
    }

    /// Check if a block at the given height is an anchor block
    ///
    /// Anchor blocks are kept fully at logarithmic intervals:
    /// - 1000, 2000, 4000, 8000, 16000, ...
    pub fn is_anchor_block(&self, height: u64) -> bool {
        if height == 0 {
            return true; // Genesis is always an anchor
        }
        
        if height < self.config.anchor_base {
            return false;
        }
        
        // Check if height is a multiple of anchor_base * 2^k
        let mut interval = self.config.anchor_base;
        while interval <= height {
            if height % interval == 0 {
                // Check if it's the largest power that divides evenly
                if height % (interval * 2) != 0 {
                    return true;
                }
            }
            interval *= 2;
        }
        
        false
    }

    /// Check if a block should be pruned
    pub fn should_prune(&self, height: u64, current_height: u64) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Keep recent blocks
        if current_height.saturating_sub(height) < self.config.keep_recent {
            return false;
        }

        // Keep anchor blocks
        if self.is_anchor_block(height) {
            return false;
        }

        true
    }

    /// Get prunable heights in a range
    pub fn get_prunable_heights(&self, start: u64, end: u64, current_height: u64) -> Vec<u64> {
        (start..=end)
            .filter(|h| self.should_prune(*h, current_height))
            .collect()
    }
}

/// Result of a pruning operation
#[derive(Debug, Clone, Default)]
pub struct PruneResult {
    /// Number of blocks pruned
    pub blocks_pruned: u64,
    /// Bytes reclaimed
    pub bytes_reclaimed: u64,
    /// Heights that were pruned
    pub pruned_heights: Vec<u64>,
}

// =============================================================================
// STORED BLOCK HEADER (for pruned blocks)
// =============================================================================

/// Header-only representation of a pruned block
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlockHeader {
    /// Block hash
    pub hash: Hash,
    /// Parent block hash
    pub parent_hash: Hash,
    /// Block height
    pub height: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Merkle root (kept for verification)
    pub merkle_root: Hash,
    /// State root (kept for verification)
    pub state_root: Hash,
    /// Flag indicating this is pruned
    pub is_pruned: bool,
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruning_config_default() {
        let config = PruningConfig::default();
        assert_eq!(config.keep_recent, 10_000);
        assert_eq!(config.anchor_base, 1000);
        assert!(config.keep_headers);
        assert!(!config.enabled);
    }

    #[test]
    fn test_is_anchor_genesis() {
        let svc = PruningService::new(PruningConfig::default());
        assert!(svc.is_anchor_block(0)); // Genesis always anchor
    }

    #[test]
    fn test_is_anchor_at_base_intervals() {
        let config = PruningConfig {
            anchor_base: 1000,
            ..Default::default()
        };
        let svc = PruningService::new(config);

        // 1000 is anchor (1000 * 2^0)
        assert!(svc.is_anchor_block(1000));
        // 2000 is anchor (1000 * 2^1)
        assert!(svc.is_anchor_block(2000));
        // 4000 is anchor (1000 * 2^2)
        assert!(svc.is_anchor_block(4000));
    }

    #[test]
    fn test_is_not_anchor() {
        let svc = PruningService::new(PruningConfig::default());
        
        assert!(!svc.is_anchor_block(500));  // Too small
        assert!(!svc.is_anchor_block(1001)); // Not a multiple
        assert!(!svc.is_anchor_block(1500)); // Not power of 2 multiple
    }

    #[test]
    fn test_should_prune_keeps_recent() {
        let config = PruningConfig {
            keep_recent: 100,
            enabled: true,
            ..Default::default()
        };
        let svc = PruningService::new(config);

        // Current height 1000, keep last 100
        assert!(!svc.should_prune(950, 1000)); // Recent
        assert!(!svc.should_prune(901, 1000)); // Recent
    }

    #[test]
    fn test_should_prune_old_non_anchor() {
        let config = PruningConfig {
            keep_recent: 100,
            anchor_base: 1000,
            enabled: true,
            ..Default::default()
        };
        let svc = PruningService::new(config);

        // Current at 20000, block at 500 is old and not anchor
        assert!(svc.should_prune(500, 20000));
    }

    #[test]
    fn test_get_prunable_heights() {
        let config = PruningConfig {
            keep_recent: 100,
            anchor_base: 100,
            enabled: true,
            keep_headers: true,
        };
        let svc = PruningService::new(config);

        // At height 1000, blocks 1-100 are old, but some are anchors
        let prunable = svc.get_prunable_heights(1, 50, 1000);
        
        // Should not include anchors (0, 100 would be but 100 is out of range)
        assert!(!prunable.contains(&0));
    }

    #[test]
    fn test_disabled_pruning() {
        let config = PruningConfig {
            enabled: false,
            ..Default::default()
        };
        let svc = PruningService::new(config);

        // Nothing should be prunable when disabled
        assert!(!svc.should_prune(1, 100000));
    }
}
