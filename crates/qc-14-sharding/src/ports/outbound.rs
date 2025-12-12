//! # Outbound Ports
//!
//! Traits for external dependencies (consensus, state, beacon chain).
//!
//! Reference: SPEC-14 Section 3.2 (Lines 225-313)

use crate::domain::{
    Address, Hash, LockData, LockProof, ShardError, ShardId, ShardStateRoot, ValidatorInfo,
};
use async_trait::async_trait;

/// Shard consensus - outbound port.
///
/// Reference: SPEC-14 Lines 228-244
#[async_trait]
pub trait ShardConsensus: Send + Sync {
    /// Get current consensus state for a shard.
    async fn get_shard_state(&self, shard_id: ShardId) -> Result<ShardStateRoot, ShardError>;

    /// Submit a transaction to shard consensus.
    async fn submit_transaction(&self, shard_id: ShardId, tx_hash: Hash) -> Result<(), ShardError>;

    /// Acquire a lock for cross-shard transaction.
    async fn acquire_lock(&self, lock_data: LockData) -> Result<LockProof, ShardError>;

    /// Release a lock.
    async fn release_lock(&self, tx_hash: Hash, shard_id: ShardId) -> Result<(), ShardError>;
}

/// Partitioned state - outbound port.
///
/// Reference: SPEC-14 Lines 247-261
#[async_trait]
pub trait PartitionedState: Send + Sync {
    /// Get account balance on a specific shard.
    async fn get_balance(&self, shard_id: ShardId, address: &Address) -> Result<u64, ShardError>;

    /// Apply a state change on a shard.
    async fn apply_change(
        &self,
        shard_id: ShardId,
        address: &Address,
        delta: i64,
    ) -> Result<(), ShardError>;

    /// Get shard state root.
    async fn get_state_root(&self, shard_id: ShardId) -> Result<Hash, ShardError>;
}

/// Beacon chain provider - outbound port.
///
/// Reference: SPEC-14 Lines 263-300
#[async_trait]
pub trait BeaconChainProvider: Send + Sync {
    /// Get validators assigned to a shard for an epoch.
    async fn get_shard_validators(
        &self,
        shard_id: ShardId,
        epoch: u64,
    ) -> Result<Vec<ValidatorInfo>, ShardError>;

    /// Get current epoch.
    async fn get_current_epoch(&self) -> Result<u64, ShardError>;

    /// Get total shard count.
    async fn get_shard_count(&self) -> Result<u16, ShardError>;

    /// Verify a cross-shard receipt.
    async fn verify_receipt(&self, receipt_hash: Hash, epoch: u64) -> Result<bool, ShardError>;
}

// =============================================================================
// Mock Implementations for Testing
// =============================================================================

/// Mock beacon chain for testing.
#[derive(Clone, Default)]
pub struct MockBeaconChain {
    /// Current epoch.
    pub epoch: u64,
    /// Shard count.
    pub shard_count: u16,
    /// Validators per shard.
    pub validators_per_shard: usize,
}

#[async_trait]
impl BeaconChainProvider for MockBeaconChain {
    async fn get_shard_validators(
        &self,
        shard_id: ShardId,
        _epoch: u64,
    ) -> Result<Vec<ValidatorInfo>, ShardError> {
        if shard_id >= self.shard_count {
            return Err(ShardError::UnknownShard(shard_id));
        }

        let validators = (0..self.validators_per_shard)
            .map(|i| ValidatorInfo {
                id: [(shard_id as u8).wrapping_add(i as u8); 32],
                stake: 32_000_000_000,
                assigned_shard: shard_id,
            })
            .collect();

        Ok(validators)
    }

    async fn get_current_epoch(&self) -> Result<u64, ShardError> {
        Ok(self.epoch)
    }

    async fn get_shard_count(&self) -> Result<u16, ShardError> {
        Ok(self.shard_count)
    }

    async fn verify_receipt(&self, _receipt_hash: Hash, _epoch: u64) -> Result<bool, ShardError> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_beacon_chain_validators() {
        let beacon = MockBeaconChain {
            epoch: 10,
            shard_count: 4,
            validators_per_shard: 128,
        };

        let validators = beacon.get_shard_validators(0, 10).await.unwrap();
        assert_eq!(validators.len(), 128);
    }

    #[tokio::test]
    async fn test_mock_beacon_chain_unknown_shard() {
        let beacon = MockBeaconChain {
            epoch: 10,
            shard_count: 4,
            validators_per_shard: 128,
        };

        let result = beacon.get_shard_validators(99, 10).await;
        assert!(matches!(result, Err(ShardError::UnknownShard(99))));
    }

    #[tokio::test]
    async fn test_mock_beacon_chain_epoch() {
        let beacon = MockBeaconChain {
            epoch: 42,
            ..Default::default()
        };

        assert_eq!(beacon.get_current_epoch().await.unwrap(), 42);
    }
}
