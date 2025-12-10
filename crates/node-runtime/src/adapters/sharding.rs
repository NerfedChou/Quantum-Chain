//! # Sharding Adapter
//!
//! Connects qc-14 Sharding to the event bus for IPC.
//!
//! ## IPC Interactions (IPC-MATRIX.md)
//!
//! | From | To | Message |
//! |------|-----|---------|
//! | 14 → 8 | Consensus | Shard consensus |
//! | 14 → 4 | State Management | Partitioned state |

#[cfg(feature = "qc-14")]
use qc_14_sharding::{
    ShardConfig, ShardId, Address, Hash,
    assign_shard, rendezvous_assign, is_cross_shard, get_involved_shards,
    compute_global_state_root, ShardStateRoot, GlobalStateRoot,
};

#[cfg(feature = "qc-14")]
use std::collections::HashMap;

/// Sharding adapter for event bus integration.
#[cfg(feature = "qc-14")]
pub struct ShardingAdapter {
    /// Configuration.
    config: ShardConfig,
    /// Subsystem ID.
    subsystem_id: u8,
    /// Cached shard assignments.
    assignment_cache: HashMap<Address, ShardId>,
}

#[cfg(feature = "qc-14")]
impl ShardingAdapter {
    /// Create a new sharding adapter.
    pub fn new(config: ShardConfig) -> Self {
        Self {
            config,
            subsystem_id: 14,
            assignment_cache: HashMap::new(),
        }
    }

    /// Create with default config.
    pub fn with_defaults() -> Self {
        Self::new(ShardConfig::default())
    }

    /// Get subsystem ID.
    pub fn subsystem_id(&self) -> u8 {
        self.subsystem_id
    }

    /// Get shard for an address.
    pub fn get_shard(&mut self, address: &Address) -> ShardId {
        if let Some(&shard) = self.assignment_cache.get(address) {
            return shard;
        }

        let shard = assign_shard(address, self.config.shard_count);
        self.assignment_cache.insert(*address, shard);
        shard
    }

    /// Check if transaction is cross-shard.
    pub fn is_cross_shard(&self, sender: &Address, recipients: &[Address]) -> bool {
        is_cross_shard(sender, recipients, self.config.shard_count)
    }

    /// Get all shards involved in a transaction.
    pub fn get_involved_shards(&self, sender: &Address, recipients: &[Address]) -> Vec<ShardId> {
        get_involved_shards(sender, recipients, self.config.shard_count)
    }

    /// Get shard count.
    pub fn shard_count(&self) -> u16 {
        self.config.shard_count
    }

    /// Clear assignment cache.
    pub fn clear_cache(&mut self) {
        self.assignment_cache.clear();
    }
}

#[cfg(feature = "qc-14")]
impl Default for ShardingAdapter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(all(test, feature = "qc-14"))]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = ShardingAdapter::with_defaults();
        assert_eq!(adapter.subsystem_id(), 14);
        assert_eq!(adapter.shard_count(), 16);
    }

    #[test]
    fn test_adapter_get_shard() {
        let mut adapter = ShardingAdapter::with_defaults();
        let addr = [42u8; 20];
        let shard = adapter.get_shard(&addr);
        assert!(shard < 16);
    }

    #[test]
    fn test_adapter_get_shard_cached() {
        let mut adapter = ShardingAdapter::with_defaults();
        let addr = [42u8; 20];
        let shard1 = adapter.get_shard(&addr);
        let shard2 = adapter.get_shard(&addr);
        assert_eq!(shard1, shard2);
    }

    #[test]
    fn test_adapter_is_cross_shard() {
        let adapter = ShardingAdapter::with_defaults();
        let sender = [1u8; 20];
        let recipient = [100u8; 20];
        // May or may not be cross-shard
        let _ = adapter.is_cross_shard(&sender, &[recipient]);
    }
}
