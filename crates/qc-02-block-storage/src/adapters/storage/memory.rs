use crate::domain::errors::KVStoreError;
use crate::ports::outbound::{BatchOperation, KeyValueStore, ScanResult};
use std::collections::HashMap;

/// In-memory key-value store for unit tests.
///
/// Provides atomic batch writes via single-threaded HashMap.
/// Production uses `RocksDbStore` with true atomic transactions.
#[derive(Default)]
pub struct InMemoryKVStore {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl InMemoryKVStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KeyValueStore for InMemoryKVStore {
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, KVStoreError> {
        Ok(self.data.get(key).cloned())
    }

    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<(), KVStoreError> {
        self.data.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), KVStoreError> {
        self.data.remove(key);
        Ok(())
    }

    fn atomic_batch_write(&mut self, operations: Vec<BatchOperation>) -> Result<(), KVStoreError> {
        // For in-memory, we can just apply all operations
        for op in operations {
            match op {
                BatchOperation::Put { key, value } => {
                    self.data.insert(key, value);
                }
                BatchOperation::Delete { key } => {
                    self.data.remove(&key);
                }
            }
        }
        Ok(())
    }

    fn exists(&self, key: &[u8]) -> Result<bool, KVStoreError> {
        Ok(self.data.contains_key(key))
    }

    fn prefix_scan(&self, prefix: &[u8]) -> Result<ScanResult, KVStoreError> {
        let results: Vec<_> = self
            .data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_kv_store() {
        let mut store = InMemoryKVStore::new();

        store.put(b"key1", b"value1").unwrap();
        store.put(b"key2", b"value2").unwrap();

        assert_eq!(store.get(b"key1").unwrap(), Some(b"value1".to_vec()));
        assert_eq!(store.get(b"key2").unwrap(), Some(b"value2".to_vec()));
        assert_eq!(store.get(b"key3").unwrap(), None);

        assert!(store.exists(b"key1").unwrap());
        assert!(!store.exists(b"key3").unwrap());
    }

    #[test]
    fn test_in_memory_kv_batch_write() {
        let mut store = InMemoryKVStore::new();

        let ops = vec![
            BatchOperation::put(b"a", b"1"),
            BatchOperation::put(b"b", b"2"),
            BatchOperation::put(b"c", b"3"),
        ];

        store.atomic_batch_write(ops).unwrap();

        assert_eq!(store.get(b"a").unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get(b"b").unwrap(), Some(b"2".to_vec()));
        assert_eq!(store.get(b"c").unwrap(), Some(b"3".to_vec()));
    }

    #[test]
    fn test_prefix_scan() {
        let mut store = InMemoryKVStore::new();

        store.put(b"block:1", b"data1").unwrap();
        store.put(b"block:2", b"data2").unwrap();
        store.put(b"height:1", b"hash1").unwrap();

        let blocks = store.prefix_scan(b"block:").unwrap();
        assert_eq!(blocks.len(), 2);

        let heights = store.prefix_scan(b"height:").unwrap();
        assert_eq!(heights.len(), 1);
    }
}
