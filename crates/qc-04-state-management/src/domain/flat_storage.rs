//! # Flat Storage Overlay (O(1) Execution)
//!
//! Dual-path storage for fast reads during transaction execution.
//!
//! ## Problem
//!
//! Merkle Tries are O(logN) for reads (disk seeks).
//! Transaction execution is read-heavy.
//!
//! ## Solution: Dual-Path Storage
//!
//! - **Trie**: For root computation and proofs
//! - **Flat DB**: For O(1) execution reads
//!
//! ## Algorithm: Materialized View
//!
//! The Flat DB is a denormalized view of the Trie.
//! Writes go to both; reads during execution only use Flat DB.

use super::{AccountState, Address, Hash, StorageKey, StorageValue};
use std::collections::HashMap;

/// Key for flat storage: Address + optional storage slot.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FlatKey {
    pub address: Address,
    pub slot: Option<StorageKey>,
}

impl FlatKey {
    /// Key for account state.
    pub fn account(address: Address) -> Self {
        Self { address, slot: None }
    }

    /// Key for storage slot.
    pub fn storage(address: Address, slot: StorageKey) -> Self {
        Self { address, slot: Some(slot) }
    }

    /// Serialize to bytes for DB key.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(52);
        bytes.extend_from_slice(&self.address);
        if let Some(slot) = &self.slot {
            bytes.extend_from_slice(slot);
        }
        bytes
    }
}

/// Value in flat storage.
#[derive(Clone, Debug)]
pub enum FlatValue {
    Account(AccountState),
    Storage(StorageValue),
}

/// Flat Storage Overlay for O(1) reads.
///
/// ## Dual-Path Storage
///
/// - Write Path: Update both Trie and Flat DB
/// - Read Path (Execution): Query Flat DB only
/// - Consistency: Flat DB = Materialized View of Trie
pub struct FlatStorage {
    /// In-memory flat storage (production would use RocksDB column family)
    data: HashMap<FlatKey, FlatValue>,
    /// Current state root (for consistency check)
    state_root: Hash,
    /// Block height of last update
    height: u64,
}

impl FlatStorage {
    /// Create empty flat storage.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            state_root: [0; 32],
            height: 0,
        }
    }

    /// Set state root and height after trie update.
    pub fn set_root(&mut self, root: Hash, height: u64) {
        self.state_root = root;
        self.height = height;
    }

    /// Get current state root.
    pub fn state_root(&self) -> Hash {
        self.state_root
    }

    // =========================================================================
    // WRITE PATH (called after trie update)
    // =========================================================================

    /// Write account state to flat storage.
    pub fn put_account(&mut self, address: Address, state: AccountState) {
        let key = FlatKey::account(address);
        self.data.insert(key, FlatValue::Account(state));
    }

    /// Write storage slot to flat storage.
    pub fn put_storage(&mut self, address: Address, slot: StorageKey, value: StorageValue) {
        let key = FlatKey::storage(address, slot);
        self.data.insert(key, FlatValue::Storage(value));
    }

    /// Delete storage slot from flat storage.
    pub fn delete_storage(&mut self, address: Address, slot: StorageKey) {
        let key = FlatKey::storage(address, slot);
        self.data.remove(&key);
    }

    // =========================================================================
    // READ PATH (O(1) - used during execution)
    // =========================================================================

    /// Get account state - O(1).
    pub fn get_account(&self, address: &Address) -> Option<&AccountState> {
        let key = FlatKey::account(*address);
        match self.data.get(&key) {
            Some(FlatValue::Account(state)) => Some(state),
            _ => None,
        }
    }

    /// Get balance - O(1).
    pub fn get_balance(&self, address: &Address) -> u128 {
        self.get_account(address)
            .map(|a| a.balance)
            .unwrap_or(0)
    }

    /// Get nonce - O(1).
    pub fn get_nonce(&self, address: &Address) -> u64 {
        self.get_account(address)
            .map(|a| a.nonce)
            .unwrap_or(0)
    }

    /// Get storage value - O(1).
    pub fn get_storage(&self, address: &Address, slot: &StorageKey) -> Option<StorageValue> {
        let key = FlatKey::storage(*address, *slot);
        match self.data.get(&key) {
            Some(FlatValue::Storage(value)) => Some(*value),
            _ => None,
        }
    }

    /// Check if account exists - O(1).
    pub fn exists(&self, address: &Address) -> bool {
        let key = FlatKey::account(*address);
        self.data.contains_key(&key)
    }

    /// Get statistics.
    pub fn stats(&self) -> FlatStorageStats {
        let accounts = self.data.keys()
            .filter(|k| k.slot.is_none())
            .count();
        let slots = self.data.len() - accounts;
        
        FlatStorageStats {
            accounts,
            storage_slots: slots,
            height: self.height,
        }
    }
}

impl Default for FlatStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for monitoring.
#[derive(Clone, Debug)]
pub struct FlatStorageStats {
    pub accounts: usize,
    pub storage_slots: usize,
    pub height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_key_account() {
        let key = FlatKey::account([0x01; 20]);
        assert!(key.slot.is_none());
        assert_eq!(key.to_bytes().len(), 20);
    }

    #[test]
    fn test_flat_key_storage() {
        let key = FlatKey::storage([0x01; 20], [0x02; 32]);
        assert!(key.slot.is_some());
        assert_eq!(key.to_bytes().len(), 52);
    }

    #[test]
    fn test_put_get_account() {
        let mut storage = FlatStorage::new();
        let addr = [0x01; 20];
        let state = AccountState::new(1000);
        
        storage.put_account(addr, state);
        
        let retrieved = storage.get_account(&addr);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().balance, 1000);
    }

    #[test]
    fn test_get_balance_o1() {
        let mut storage = FlatStorage::new();
        let addr = [0x01; 20];
        storage.put_account(addr, AccountState::new(5000));
        
        // O(1) balance lookup
        assert_eq!(storage.get_balance(&addr), 5000);
        assert_eq!(storage.get_balance(&[0xFF; 20]), 0);
    }

    #[test]
    fn test_put_get_storage() {
        let mut storage = FlatStorage::new();
        let addr = [0x01; 20];
        let slot = [0x02; 32];
        let value = [0x03; 32];
        
        storage.put_storage(addr, slot, value);
        
        let retrieved = storage.get_storage(&addr, &slot);
        assert_eq!(retrieved, Some(value));
    }

    #[test]
    fn test_delete_storage() {
        let mut storage = FlatStorage::new();
        let addr = [0x01; 20];
        let slot = [0x02; 32];
        
        storage.put_storage(addr, slot, [0xFF; 32]);
        assert!(storage.get_storage(&addr, &slot).is_some());
        
        storage.delete_storage(addr, slot);
        assert!(storage.get_storage(&addr, &slot).is_none());
    }

    #[test]
    fn test_stats() {
        let mut storage = FlatStorage::new();
        
        // Add 2 accounts and 3 storage slots
        storage.put_account([0x01; 20], AccountState::new(100));
        storage.put_account([0x02; 20], AccountState::new(200));
        storage.put_storage([0x01; 20], [0xAA; 32], [0xBB; 32]);
        storage.put_storage([0x01; 20], [0xCC; 32], [0xDD; 32]);
        storage.put_storage([0x02; 20], [0xEE; 32], [0xFF; 32]);
        
        let stats = storage.stats();
        assert_eq!(stats.accounts, 2);
        assert_eq!(stats.storage_slots, 3);
    }
}
