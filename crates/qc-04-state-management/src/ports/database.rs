use crate::domain::{Hash, StateError};

/// Trie database abstraction
pub trait TrieDatabase: Send + Sync {
    fn get_node(&self, hash: &Hash) -> Result<Option<Vec<u8>>, StateError>;
    fn put_node(&self, hash: Hash, data: Vec<u8>) -> Result<(), StateError>;
    fn batch_put(&self, nodes: Vec<(Hash, Vec<u8>)>) -> Result<(), StateError>;
    fn delete_node(&self, hash: &Hash) -> Result<(), StateError>;
}

/// Snapshot storage abstraction
pub trait SnapshotStorage: Send + Sync {
    fn create_snapshot(&self, height: u64, root: Hash) -> Result<(), StateError>;
    fn get_nearest_snapshot(&self, height: u64) -> Result<Option<(u64, Hash)>, StateError>;
    fn prune_snapshots(&self, keep_after: u64) -> Result<u64, StateError>;
}
