//! # Transient Storage (EIP-1153)
//!
//! Implements transient storage opcodes TLOAD and TSTORE.
//! Transient storage is discarded at the end of the transaction.

use crate::domain::value_objects::{Address, StorageKey, StorageValue};
use std::collections::HashMap;

/// Transient storage for a single transaction.
///
/// Per EIP-1153, transient storage:
/// - Is cleared at the end of each transaction
/// - Does NOT persist to state
/// - Has cheaper gas costs than regular storage
/// - Is useful for reentrancy locks and temporary data
#[derive(Debug, Default, Clone)]
pub struct TransientStorage {
    /// Storage per contract address.
    data: HashMap<Address, HashMap<StorageKey, StorageValue>>,
}

impl TransientStorage {
    /// Creates a new empty transient storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Load a value from transient storage (TLOAD).
    ///
    /// Returns zero if the key has not been set.
    #[must_use]
    pub fn tload(&self, address: &Address, key: &StorageKey) -> StorageValue {
        self.data
            .get(address)
            .and_then(|storage| storage.get(key))
            .copied()
            .unwrap_or_default()
    }

    /// Store a value in transient storage (TSTORE).
    pub fn tstore(&mut self, address: Address, key: StorageKey, value: StorageValue) {
        self.data
            .entry(address)
            .or_default()
            .insert(key, value);
    }

    /// Clear all transient storage (called at end of transaction).
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Get the number of contracts with transient storage.
    #[must_use]
    pub fn contract_count(&self) -> usize {
        self.data.len()
    }

    /// Get the total number of storage slots across all contracts.
    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.data.values().map(HashMap::len).sum()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address() -> Address {
        let mut bytes = [0u8; 20];
        bytes[18..20].copy_from_slice(&0x1234u16.to_be_bytes());
        Address::new(bytes)
    }

    fn addr_from_u64(n: u64) -> Address {
        let mut bytes = [0u8; 20];
        bytes[12..20].copy_from_slice(&n.to_be_bytes());
        Address::new(bytes)
    }

    fn test_key(n: u64) -> StorageKey {
        let mut key = [0u8; 32];
        key[24..32].copy_from_slice(&n.to_be_bytes());
        StorageKey(key)
    }

    fn test_value(n: u64) -> StorageValue {
        let mut value = [0u8; 32];
        value[24..32].copy_from_slice(&n.to_be_bytes());
        StorageValue(value)
    }

    #[test]
    fn test_tload_unset_returns_zero() {
        let storage = TransientStorage::new();
        let addr = test_address();
        let key = test_key(1);

        let value = storage.tload(&addr, &key);
        assert_eq!(value, StorageValue::default());
    }

    #[test]
    fn test_tstore_and_tload() {
        let mut storage = TransientStorage::new();
        let addr = test_address();
        let key = test_key(1);
        let value = test_value(42);

        storage.tstore(addr, key, value);
        let loaded = storage.tload(&addr, &key);

        assert_eq!(loaded, value);
    }

    #[test]
    fn test_tstore_overwrite() {
        let mut storage = TransientStorage::new();
        let addr = test_address();
        let key = test_key(1);

        storage.tstore(addr, key, test_value(1));
        storage.tstore(addr, key, test_value(2));

        let loaded = storage.tload(&addr, &key);
        assert_eq!(loaded, test_value(2));
    }

    #[test]
    fn test_clear() {
        let mut storage = TransientStorage::new();
        let addr = test_address();

        storage.tstore(addr, test_key(1), test_value(1));
        storage.tstore(addr, test_key(2), test_value(2));

        assert_eq!(storage.slot_count(), 2);

        storage.clear();

        assert_eq!(storage.slot_count(), 0);
        assert_eq!(storage.tload(&addr, &test_key(1)), StorageValue::default());
    }

    #[test]
    fn test_multiple_contracts() {
        let mut storage = TransientStorage::new();
        let addr1 = addr_from_u64(1);
        let addr2 = addr_from_u64(2);
        let key = test_key(1);

        storage.tstore(addr1, key, test_value(100));
        storage.tstore(addr2, key, test_value(200));

        assert_eq!(storage.contract_count(), 2);
        assert_eq!(storage.tload(&addr1, &key), test_value(100));
        assert_eq!(storage.tload(&addr2, &key), test_value(200));
    }

    #[test]
    fn test_isolation_between_contracts() {
        let mut storage = TransientStorage::new();
        let addr1 = addr_from_u64(1);
        let addr2 = addr_from_u64(2);
        let key = test_key(1);

        storage.tstore(addr1, key, test_value(42));

        // addr2 should not see addr1's storage
        assert_eq!(storage.tload(&addr2, &key), StorageValue::default());
    }
}
