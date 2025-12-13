//! Partitioned State Adapter
//!
//! Implements `PartitionedState` port for shard-local state access.
//! Reference: SPEC-14 Section 3.2

use crate::domain::{Address, Hash, ShardError, ShardId};
use crate::ports::outbound::PartitionedState;
use async_trait::async_trait;
use parking_lot::RwLock;
use sha3::{Digest, Keccak256};
use std::collections::HashMap;
use tracing::debug;

/// In-memory partitioned state for testing.
///
/// In production, this would connect to shard-specific storage.
pub struct InMemoryPartitionedState {
    /// Balances per shard: shard_id -> (address -> balance).
    balances: RwLock<HashMap<ShardId, HashMap<Address, u64>>>,
    /// State roots per shard.
    state_roots: RwLock<HashMap<ShardId, Hash>>,
}

impl InMemoryPartitionedState {
    /// Create a new empty state.
    pub fn new() -> Self {
        Self {
            balances: RwLock::new(HashMap::new()),
            state_roots: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize with shard count.
    pub fn with_shards(shard_count: u16) -> Self {
        let state = Self::new();

        let mut balances = state.balances.write();
        let mut roots = state.state_roots.write();

        for shard_id in 0..shard_count {
            balances.insert(shard_id, HashMap::new());
            roots.insert(shard_id, [0u8; 32]);
        }

        drop(balances);
        drop(roots);
        state
    }

    /// Set initial balance for testing.
    pub fn set_balance(&self, shard_id: ShardId, address: Address, balance: u64) {
        self.balances
            .write()
            .entry(shard_id)
            .or_default()
            .insert(address, balance);
    }

    /// Update state root after changes (uses Keccak256).
    fn update_state_root(&self, shard_id: ShardId) {
        let balances = self.balances.read();
        if let Some(shard_balances) = balances.get(&shard_id) {
            let mut hasher = Keccak256::new();
            hasher.update(shard_id.to_le_bytes());

            // Sort for deterministic hashing
            let mut entries: Vec<_> = shard_balances.iter().collect();
            entries.sort_by_key(|(addr, _)| *addr);

            for (addr, balance) in entries {
                hasher.update(addr);
                hasher.update(balance.to_le_bytes());
            }

            let result = hasher.finalize();
            let mut root = [0u8; 32];
            root.copy_from_slice(&result);

            drop(balances);
            self.state_roots.write().insert(shard_id, root);
        }
    }
}

impl Default for InMemoryPartitionedState {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PartitionedState for InMemoryPartitionedState {
    async fn get_balance(&self, shard_id: ShardId, address: &Address) -> Result<u64, ShardError> {
        debug!(
            "[qc-14] Getting balance for {:02x}{:02x}... on shard {}",
            address[0], address[1], shard_id
        );

        let balances = self.balances.read();
        let shard_balances = balances
            .get(&shard_id)
            .ok_or(ShardError::UnknownShard(shard_id))?;

        Ok(*shard_balances.get(address).unwrap_or(&0))
    }

    async fn apply_change(
        &self,
        shard_id: ShardId,
        address: &Address,
        delta: i64,
    ) -> Result<(), ShardError> {
        debug!(
            "[qc-14] Applying balance change {} to {:02x}{:02x}... on shard {}",
            delta, address[0], address[1], shard_id
        );

        let mut balances = self.balances.write();
        let shard_balances = balances
            .get_mut(&shard_id)
            .ok_or(ShardError::UnknownShard(shard_id))?;

        let current = *shard_balances.get(address).unwrap_or(&0);
        let new_balance = if delta >= 0 {
            current.saturating_add(delta as u64)
        } else {
            let abs_delta = delta.unsigned_abs();
            if abs_delta > current {
                return Err(ShardError::StateInconsistency(format!(
                    "Insufficient balance: {} < {} for {:02x}{:02x}...",
                    current, abs_delta, address[0], address[1]
                )));
            }
            current - abs_delta
        };

        shard_balances.insert(*address, new_balance);

        // Update state root
        drop(balances);
        self.update_state_root(shard_id);

        Ok(())
    }

    async fn get_state_root(&self, shard_id: ShardId) -> Result<Hash, ShardError> {
        self.state_roots
            .read()
            .get(&shard_id)
            .cloned()
            .ok_or(ShardError::UnknownShard(shard_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_balance_empty() {
        let state = InMemoryPartitionedState::with_shards(4);
        let balance = state.get_balance(0, &[1u8; 20]).await.unwrap();

        assert_eq!(balance, 0);
    }

    #[tokio::test]
    async fn test_set_and_get_balance() {
        let state = InMemoryPartitionedState::with_shards(4);
        state.set_balance(0, [1u8; 20], 1000);

        let balance = state.get_balance(0, &[1u8; 20]).await.unwrap();
        assert_eq!(balance, 1000);
    }

    #[tokio::test]
    async fn test_apply_positive_change() {
        let state = InMemoryPartitionedState::with_shards(4);
        state.set_balance(0, [1u8; 20], 1000);

        state.apply_change(0, &[1u8; 20], 500).await.unwrap();

        let balance = state.get_balance(0, &[1u8; 20]).await.unwrap();
        assert_eq!(balance, 1500);
    }

    #[tokio::test]
    async fn test_apply_negative_change() {
        let state = InMemoryPartitionedState::with_shards(4);
        state.set_balance(0, [1u8; 20], 1000);

        state.apply_change(0, &[1u8; 20], -300).await.unwrap();

        let balance = state.get_balance(0, &[1u8; 20]).await.unwrap();
        assert_eq!(balance, 700);
    }

    #[tokio::test]
    async fn test_insufficient_balance_fails() {
        let state = InMemoryPartitionedState::with_shards(4);
        state.set_balance(0, [1u8; 20], 100);

        let result = state.apply_change(0, &[1u8; 20], -500).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_state_root_changes() {
        let state = InMemoryPartitionedState::with_shards(4);

        let root1 = state.get_state_root(0).await.unwrap();
        state.set_balance(0, [1u8; 20], 1000);
        state.apply_change(0, &[1u8; 20], 1).await.unwrap();
        let root2 = state.get_state_root(0).await.unwrap();

        assert_ne!(root1, root2);
    }

    #[tokio::test]
    async fn test_unknown_shard_fails() {
        let state = InMemoryPartitionedState::with_shards(4);

        let result = state.get_balance(99, &[1u8; 20]).await;
        assert!(result.is_err());
    }
}

