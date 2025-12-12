//! # Inbound Ports
//!
//! API trait defining what the Sharding subsystem can do.
//!
//! Reference: SPEC-14 Section 3.1 (Lines 181-223)

use crate::domain::{
    Address, GlobalStateRoot, ShardConfig, ShardError, ShardId, ValidatorInfo,
};
use async_trait::async_trait;

/// Routing result for a transaction.
#[derive(Clone, Debug)]
pub struct RoutingResult {
    /// Is this a cross-shard transaction?
    pub is_cross_shard: bool,
    /// Source shard.
    pub source_shard: ShardId,
    /// Target shards (empty if same-shard).
    pub target_shards: Vec<ShardId>,
}

/// Sharding API - inbound port.
///
/// Reference: SPEC-14 Lines 181-223
#[async_trait]
pub trait ShardingApi: Send + Sync {
    /// Get shard ID for an address.
    fn get_shard(&self, address: &Address) -> ShardId;

    /// Route a transaction to appropriate shard(s).
    fn route_transaction(&self, sender: &Address, recipients: &[Address]) -> RoutingResult;

    /// Get global state root.
    async fn get_global_state_root(&self) -> Result<GlobalStateRoot, ShardError>;

    /// Get validators for a shard.
    async fn get_shard_validators(
        &self,
        shard_id: ShardId,
    ) -> Result<Vec<ValidatorInfo>, ShardError>;

    /// Get current shard configuration.
    fn get_config(&self) -> &ShardConfig;

    /// Get total shard count.
    fn shard_count(&self) -> u16;
}
