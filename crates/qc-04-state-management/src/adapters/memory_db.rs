use crate::domain::{Hash, StateError};
use crate::ports::{SnapshotStorage, TrieDatabase};
use std::collections::HashMap;
use std::sync::RwLock;

/// In-memory implementation of TrieDatabase for testing
pub struct InMemoryTrieDb {
    nodes: RwLock<HashMap<Hash, Vec<u8>>>,
}

impl InMemoryTrieDb {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryTrieDb {
    fn default() -> Self {
        Self::new()
    }
}

impl TrieDatabase for InMemoryTrieDb {
    fn get_node(&self, hash: &Hash) -> Result<Option<Vec<u8>>, StateError> {
        let nodes = self
            .nodes
            .read()
            .map_err(|_| StateError::LockPoisoned)?;
        Ok(nodes.get(hash).cloned())
    }

    fn put_node(&self, hash: Hash, data: Vec<u8>) -> Result<(), StateError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| StateError::LockPoisoned)?;
        nodes.insert(hash, data);
        Ok(())
    }

    fn batch_put(&self, batch: Vec<(Hash, Vec<u8>)>) -> Result<(), StateError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| StateError::LockPoisoned)?;
        for (hash, data) in batch {
            nodes.insert(hash, data);
        }
        Ok(())
    }

    fn delete_node(&self, hash: &Hash) -> Result<(), StateError> {
        let mut nodes = self
            .nodes
            .write()
            .map_err(|_| StateError::LockPoisoned)?;
        nodes.remove(hash);
        Ok(())
    }
}

/// In-memory implementation of SnapshotStorage for testing
pub struct InMemorySnapshotStorage {
    snapshots: RwLock<HashMap<u64, Hash>>,
}

impl InMemorySnapshotStorage {
    pub fn new() -> Self {
        Self {
            snapshots: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySnapshotStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotStorage for InMemorySnapshotStorage {
    fn create_snapshot(&self, height: u64, root: Hash) -> Result<(), StateError> {
        let mut snapshots = self
            .snapshots
            .write()
            .map_err(|_| StateError::LockPoisoned)?;
        snapshots.insert(height, root);
        Ok(())
    }

    fn get_nearest_snapshot(&self, height: u64) -> Result<Option<(u64, Hash)>, StateError> {
        let snapshots = self
            .snapshots
            .read()
            .map_err(|_| StateError::LockPoisoned)?;

        // Find the nearest snapshot at or before the given height
        let nearest = snapshots
            .iter()
            .filter(|(h, _)| **h <= height)
            .max_by_key(|(h, _)| *h)
            .map(|(h, root)| (*h, *root));

        Ok(nearest)
    }

    fn prune_snapshots(&self, keep_after: u64) -> Result<u64, StateError> {
        let mut snapshots = self
            .snapshots
            .write()
            .map_err(|_| StateError::LockPoisoned)?;
        let before = snapshots.len();
        snapshots.retain(|h, _| *h >= keep_after);
        Ok((before - snapshots.len()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_db_operations() {
        let db = InMemoryTrieDb::new();
        let hash = [0xAB; 32];
        let data = vec![1, 2, 3, 4];

        // Put
        db.put_node(hash, data.clone()).unwrap();

        // Get
        let retrieved = db.get_node(&hash).unwrap();
        assert_eq!(retrieved, Some(data));

        // Delete
        db.delete_node(&hash).unwrap();
        let retrieved = db.get_node(&hash).unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_snapshot_storage() {
        let storage = InMemorySnapshotStorage::new();

        storage.create_snapshot(100, [0x01; 32]).unwrap();
        storage.create_snapshot(200, [0x02; 32]).unwrap();
        storage.create_snapshot(300, [0x03; 32]).unwrap();

        // Get nearest at 250
        let (height, root) = storage.get_nearest_snapshot(250).unwrap().unwrap();
        assert_eq!(height, 200);
        assert_eq!(root, [0x02; 32]);

        // Prune old snapshots
        let pruned = storage.prune_snapshots(150).unwrap();
        assert_eq!(pruned, 1); // Removed snapshot at 100
    }
}
