//! # Pruning Security
//!
//! Security controls for pruning operations.
//!
//! ## Security Invariants
//!
//! - Genesis block must never be pruned
//! - Finalized blocks require special handling
//! - Reorg protection during pruning

/// Minimum blocks to keep for reorg protection.
pub const MIN_REORG_PROTECTION_BLOCKS: u64 = 100;

/// Validate that a height is safe to prune (not genesis, not too recent).
pub fn validate_prune_height(height: u64, current_height: u64) -> Result<(), &'static str> {
    if height == 0 {
        return Err("Cannot prune genesis block");
    }

    let depth = current_height.saturating_sub(height);
    if depth < MIN_REORG_PROTECTION_BLOCKS {
        return Err("Block too recent for safe pruning");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_cannot_be_pruned() {
        assert!(validate_prune_height(0, 1000).is_err());
    }

    #[test]
    fn test_recent_cannot_be_pruned() {
        assert!(validate_prune_height(950, 1000).is_err());
    }

    #[test]
    fn test_old_block_can_be_pruned() {
        assert!(validate_prune_height(100, 1000).is_ok());
    }
}
