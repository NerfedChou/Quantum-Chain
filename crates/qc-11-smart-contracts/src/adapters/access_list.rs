//! # Access List Adapter
//!
//! Implementation of EIP-2929 warm/cold account and storage tracking.

use crate::domain::value_objects::{Address, StorageKey};
use crate::ports::outbound::{AccessList, AccessStatus};
use std::collections::{HashMap, HashSet};

/// In-memory access list implementation.
#[derive(Clone, Debug, Default)]
pub struct InMemoryAccessList {
    /// Warm accounts.
    warm_accounts: HashSet<Address>,
    /// Warm storage slots (address -> set of keys).
    warm_storage: HashMap<Address, HashSet<StorageKey>>,
}

impl InMemoryAccessList {
    /// Create a new empty access list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-warmed accounts (e.g., from EIP-2930 access list transaction).
    #[must_use]
    pub fn with_prewarmed(
        accounts: impl IntoIterator<Item = Address>,
        storage: impl IntoIterator<Item = (Address, StorageKey)>,
    ) -> Self {
        let mut list = Self::new();

        for addr in accounts {
            list.warm_accounts.insert(addr);
        }

        for (addr, key) in storage {
            list.warm_storage
                .entry(addr)
                .or_insert_with(HashSet::new)
                .insert(key);
        }

        list
    }

    /// Pre-warm standard addresses (precompiles + tx sender/recipient).
    pub fn prewarm_standard(&mut self, origin: Address, recipient: Address) {
        // Precompiles (0x01 - 0x09) are always warm
        for i in 1..=9 {
            let mut addr = [0u8; 20];
            addr[19] = i;
            self.warm_accounts.insert(Address::new(addr));
        }

        // Origin (tx sender) is always warm
        self.warm_accounts.insert(origin);

        // Recipient is always warm
        self.warm_accounts.insert(recipient);
    }

    /// Clear all access tracking.
    pub fn clear(&mut self) {
        self.warm_accounts.clear();
        self.warm_storage.clear();
    }
}

impl AccessList for InMemoryAccessList {
    fn touch_account(&mut self, address: Address) -> AccessStatus {
        if self.warm_accounts.contains(&address) {
            AccessStatus::Warm
        } else {
            self.warm_accounts.insert(address);
            AccessStatus::Cold
        }
    }

    fn touch_storage(&mut self, address: Address, key: StorageKey) -> AccessStatus {
        // Account access is implicit
        self.warm_accounts.insert(address);

        let slots = self
            .warm_storage
            .entry(address)
            .or_insert_with(HashSet::new);
        if slots.contains(&key) {
            AccessStatus::Warm
        } else {
            slots.insert(key);
            AccessStatus::Cold
        }
    }

    fn is_account_warm(&self, address: Address) -> bool {
        self.warm_accounts.contains(&address)
    }

    fn is_storage_warm(&self, address: Address, key: StorageKey) -> bool {
        self.warm_storage
            .get(&address)
            .map(|slots| slots.contains(&key))
            .unwrap_or(false)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_access() {
        let mut list = InMemoryAccessList::new();
        let addr = Address::new([1u8; 20]);

        // First access is cold
        assert_eq!(list.touch_account(addr), AccessStatus::Cold);
        assert!(list.is_account_warm(addr));

        // Second access is warm
        assert_eq!(list.touch_account(addr), AccessStatus::Warm);
    }

    #[test]
    fn test_storage_access() {
        let mut list = InMemoryAccessList::new();
        let addr = Address::new([1u8; 20]);
        let key = StorageKey::new([0u8; 32]);

        // First access is cold
        assert_eq!(list.touch_storage(addr, key), AccessStatus::Cold);
        assert!(list.is_storage_warm(addr, key));

        // Second access is warm
        assert_eq!(list.touch_storage(addr, key), AccessStatus::Warm);

        // Different key is cold
        let key2 = StorageKey::new([1u8; 32]);
        assert_eq!(list.touch_storage(addr, key2), AccessStatus::Cold);
    }

    #[test]
    fn test_prewarm_standard() {
        let mut list = InMemoryAccessList::new();
        let origin = Address::new([1u8; 20]);
        let recipient = Address::new([2u8; 20]);

        list.prewarm_standard(origin, recipient);

        // Precompiles should be warm
        let mut precompile = [0u8; 20];
        precompile[19] = 1;
        assert!(list.is_account_warm(Address::new(precompile)));

        // Origin and recipient should be warm
        assert!(list.is_account_warm(origin));
        assert!(list.is_account_warm(recipient));
    }

    #[test]
    fn test_with_prewarmed() {
        let addr1 = Address::new([1u8; 20]);
        let addr2 = Address::new([2u8; 20]);
        let key = StorageKey::new([0u8; 32]);

        let list = InMemoryAccessList::with_prewarmed(vec![addr1], vec![(addr2, key)]);

        assert!(list.is_account_warm(addr1));
        assert!(list.is_storage_warm(addr2, key));
    }
}
