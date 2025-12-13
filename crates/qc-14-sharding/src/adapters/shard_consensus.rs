//! Shard Consensus Adapter
//!
//! Implements `ShardConsensus` port for cross-shard coordination.
//! Reference: SPEC-14 Section 3.2

use crate::domain::{Hash, LockData, LockProof, ShardError, ShardId, ShardStateRoot, Signature};
use crate::ports::outbound::ShardConsensus;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use tracing::{debug, info};

/// Event bus based shard consensus adapter.
///
/// Coordinates cross-shard transactions via internal state management.
/// In production, this would publish events to coordinate with other shards.
pub struct EventBusShardConsensus {
    /// Current state roots per shard.
    shard_states: RwLock<HashMap<ShardId, ShardStateRoot>>,
    /// Active locks (tx_hash -> LockData).
    locks: RwLock<HashMap<Hash, LockData>>,
    /// Pending transactions per shard.
    pending_txs: RwLock<HashMap<ShardId, Vec<Hash>>>,
}

impl EventBusShardConsensus {
    /// Create a new adapter.
    pub fn new() -> Self {
        Self {
            shard_states: RwLock::new(HashMap::new()),
            locks: RwLock::new(HashMap::new()),
            pending_txs: RwLock::new(HashMap::new()),
        }
    }

    /// Initialize shard states.
    pub fn with_shards(shard_count: u16) -> Self {
        let adapter = Self::new();
        let mut states = adapter.shard_states.write();
        let mut pending = adapter.pending_txs.write();

        for shard_id in 0..shard_count {
            states.insert(shard_id, ShardStateRoot::new(shard_id, [0u8; 32], 0, 0));
            pending.insert(shard_id, Vec::new());
        }

        drop(states);
        drop(pending);
        adapter
    }
}

impl Default for EventBusShardConsensus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ShardConsensus for EventBusShardConsensus {
    async fn get_shard_state(&self, shard_id: ShardId) -> Result<ShardStateRoot, ShardError> {
        debug!("[qc-14] Getting state for shard {}", shard_id);

        self.shard_states
            .read()
            .get(&shard_id)
            .cloned()
            .ok_or(ShardError::UnknownShard(shard_id))
    }

    async fn submit_transaction(&self, shard_id: ShardId, tx_hash: Hash) -> Result<(), ShardError> {
        info!(
            "[qc-14] Submitting tx {:02x}{:02x}... to shard {}",
            tx_hash[0], tx_hash[1], shard_id
        );

        // Verify shard exists
        if !self.shard_states.read().contains_key(&shard_id) {
            return Err(ShardError::UnknownShard(shard_id));
        }

        // Add to pending transactions
        self.pending_txs
            .write()
            .entry(shard_id)
            .or_default()
            .push(tx_hash);

        Ok(())
    }

    async fn acquire_lock(&self, lock_data: LockData) -> Result<LockProof, ShardError> {
        info!(
            "[qc-14] Acquiring lock for tx {:02x}{:02x}... on shard {}",
            lock_data.tx_hash[0], lock_data.tx_hash[1], lock_data.shard_id
        );

        // Check for existing lock
        if self.locks.read().contains_key(&lock_data.tx_hash) {
            return Err(ShardError::LockFailed(format!(
                "Lock already exists for tx {:02x}{:02x}...",
                lock_data.tx_hash[0], lock_data.tx_hash[1]
            )));
        }

        // Create lock proof with mock signatures
        let proof = LockProof {
            lock_data: lock_data.clone(),
            merkle_proof: vec![[0u8; 32]; 3], // Mock merkle proof
            signatures: vec![
                Signature {
                    validator_id: [1u8; 32],
                    signature_bytes: vec![0u8; 64],
                },
                Signature {
                    validator_id: [2u8; 32],
                    signature_bytes: vec![0u8; 64],
                },
            ],
        };

        // Store lock
        self.locks.write().insert(lock_data.tx_hash, lock_data);

        Ok(proof)
    }

    async fn release_lock(&self, tx_hash: Hash, shard_id: ShardId) -> Result<(), ShardError> {
        debug!(
            "[qc-14] Releasing lock for tx {:02x}{:02x}... on shard {}",
            tx_hash[0], tx_hash[1], shard_id
        );

        let removed = self.locks.write().remove(&tx_hash);

        if removed.is_none() {
            return Err(ShardError::LockFailed(format!(
                "Lock not found for tx {:02x}{:02x}...",
                tx_hash[0], tx_hash[1]
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_shard_state() {
        let adapter = EventBusShardConsensus::with_shards(4);

        let state = adapter.get_shard_state(0).await.unwrap();
        assert_eq!(state.shard_id, 0);
    }

    #[tokio::test]
    async fn test_unknown_shard_fails() {
        let adapter = EventBusShardConsensus::with_shards(4);

        let result = adapter.get_shard_state(99).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_submit_transaction() {
        let adapter = EventBusShardConsensus::with_shards(4);

        let result = adapter.submit_transaction(0, [1u8; 32]).await;
        assert!(result.is_ok());

        let pending = adapter.pending_txs.read();
        assert!(pending.get(&0).unwrap().contains(&[1u8; 32]));
    }

    #[tokio::test]
    async fn test_acquire_and_release_lock() {
        let adapter = EventBusShardConsensus::with_shards(4);

        let lock_data = LockData {
            tx_hash: [1u8; 32],
            shard_id: 0,
            account: [0u8; 20],
            amount: 1000,
            expires_at: 0,
        };

        let proof = adapter.acquire_lock(lock_data).await.unwrap();
        assert_eq!(proof.lock_data.tx_hash, [1u8; 32]);
        assert!(!proof.signatures.is_empty());

        let release = adapter.release_lock([1u8; 32], 0).await;
        assert!(release.is_ok());
    }

    #[tokio::test]
    async fn test_double_lock_fails() {
        let adapter = EventBusShardConsensus::with_shards(4);

        let lock_data = LockData {
            tx_hash: [2u8; 32],
            shard_id: 0,
            account: [0u8; 20],
            amount: 1000,
            expires_at: 0,
        };

        adapter.acquire_lock(lock_data.clone()).await.unwrap();
        let result = adapter.acquire_lock(lock_data).await;
        assert!(result.is_err());
    }
}

