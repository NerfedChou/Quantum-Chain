use super::{
    AccountState, Address, Hash, StateConfig, StateError, StateProof, StorageKey, StorageValue,
    EMPTY_TRIE_ROOT,
};
use sha3::{Digest, Keccak256};
use std::collections::HashMap;

/// Nibble path for trie traversal
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nibbles(pub Vec<u8>);

impl Nibbles {
    pub fn from_address(addr: &Address) -> Self {
        let mut nibbles = Vec::with_capacity(40);
        for byte in addr {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
        Nibbles(nibbles)
    }

    pub fn from_key(key: &StorageKey) -> Self {
        let mut nibbles = Vec::with_capacity(64);
        for byte in key {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
        Nibbles(nibbles)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Node types in the Patricia Merkle Trie
#[derive(Clone, Debug)]
pub enum TrieNode {
    Empty,
    Leaf {
        path: Nibbles,
        value: Vec<u8>,
    },
    Extension {
        path: Nibbles,
        child: Hash,
    },
    Branch {
        children: Box<[Option<Hash>; 16]>,
        value: Option<Vec<u8>>,
    },
}

/// Patricia Merkle Trie for state management
pub struct PatriciaMerkleTrie {
    root: Hash,
    accounts: HashMap<Address, AccountState>,
    storage: HashMap<(Address, StorageKey), StorageValue>,
    storage_counts: HashMap<Address, usize>,
    config: StateConfig,
}

impl PatriciaMerkleTrie {
    pub fn new() -> Self {
        Self::with_config(StateConfig::default())
    }

    pub fn with_config(config: StateConfig) -> Self {
        Self {
            root: EMPTY_TRIE_ROOT,
            accounts: HashMap::new(),
            storage: HashMap::new(),
            storage_counts: HashMap::new(),
            config,
        }
    }

    pub fn root_hash(&self) -> Hash {
        self.compute_root()
    }

    fn compute_root(&self) -> Hash {
        if self.accounts.is_empty() {
            return EMPTY_TRIE_ROOT;
        }

        let mut hasher = Keccak256::new();

        // Sort addresses for deterministic ordering
        let mut sorted_accounts: Vec<_> = self.accounts.iter().collect();
        sorted_accounts.sort_by_key(|(addr, _)| *addr);

        for (addr, state) in sorted_accounts {
            hasher.update(addr);
            hasher.update(state.balance.to_be_bytes());
            hasher.update(state.nonce.to_be_bytes());
            hasher.update(state.code_hash);
            hasher.update(state.storage_root);
        }

        hasher.finalize().into()
    }

    pub fn insert_account(
        &mut self,
        address: Address,
        state: &AccountState,
    ) -> Result<(), StateError> {
        self.accounts.insert(address, state.clone());
        self.root = self.compute_root();
        Ok(())
    }

    pub fn get_account(&self, address: Address) -> Result<Option<AccountState>, StateError> {
        Ok(self.accounts.get(&address).cloned())
    }

    pub fn set_balance(&mut self, address: Address, balance: u128) -> Result<(), StateError> {
        let state = self.accounts.entry(address).or_default();
        state.balance = balance;
        self.root = self.compute_root();
        Ok(())
    }

    pub fn get_balance(&self, address: Address) -> Result<u128, StateError> {
        Ok(self.accounts.get(&address).map(|s| s.balance).unwrap_or(0))
    }

    pub fn get_nonce(&self, address: Address) -> Result<u64, StateError> {
        Ok(self.accounts.get(&address).map(|s| s.nonce).unwrap_or(0))
    }

    pub fn increment_nonce(&mut self, address: Address) -> Result<(), StateError> {
        let state = self.accounts.entry(address).or_default();
        state.nonce += 1;
        self.root = self.compute_root();
        Ok(())
    }

    pub fn set_storage(
        &mut self,
        contract: Address,
        key: StorageKey,
        value: StorageValue,
    ) -> Result<(), StateError> {
        let count = self.storage_counts.entry(contract).or_insert(0);

        // Check if this is a new slot
        if !self.storage.contains_key(&(contract, key)) {
            if *count >= self.config.max_storage_slots_per_contract {
                return Err(StateError::StorageLimitExceeded { address: contract });
            }
            *count += 1;
        }

        self.storage.insert((contract, key), value);

        // Update account storage root
        let new_root = self.compute_storage_root(contract);
        if let Some(account) = self.accounts.get_mut(&contract) {
            account.storage_root = new_root;
        }

        self.root = self.compute_root();
        Ok(())
    }

    pub fn get_storage(
        &self,
        contract: Address,
        key: StorageKey,
    ) -> Result<Option<StorageValue>, StateError> {
        Ok(self.storage.get(&(contract, key)).copied())
    }

    pub fn delete_storage(&mut self, contract: Address, key: StorageKey) -> Result<(), StateError> {
        if self.storage.remove(&(contract, key)).is_some() {
            if let Some(count) = self.storage_counts.get_mut(&contract) {
                *count = count.saturating_sub(1);
            }

            let new_root = self.compute_storage_root(contract);
            if let Some(account) = self.accounts.get_mut(&contract) {
                account.storage_root = new_root;
            }
        }

        self.root = self.compute_root();
        Ok(())
    }

    fn compute_storage_root(&self, contract: Address) -> Hash {
        let mut hasher = Keccak256::new();

        let mut slots: Vec<_> = self
            .storage
            .iter()
            .filter(|((addr, _), _)| *addr == contract)
            .collect();

        slots.sort_by_key(|((_, key), _)| *key);

        for ((_, key), value) in slots {
            hasher.update(key);
            hasher.update(value);
        }

        hasher.finalize().into()
    }

    pub fn generate_proof(&self, address: Address) -> Result<StateProof, StateError> {
        let account = self.accounts.get(&address).cloned();
        let root = self.root_hash();

        // Generate proof nodes (simplified for now)
        let proof_nodes = if account.is_some() {
            vec![address.to_vec()]
        } else {
            vec![]
        };

        Ok(StateProof {
            address,
            account_state: account,
            proof_nodes,
            state_root: root,
        })
    }

    /// Apply a balance change with validation
    pub fn apply_balance_change(
        &mut self,
        address: Address,
        delta: i128,
    ) -> Result<(), StateError> {
        let current = self.get_balance(address)?;

        let new_balance = if delta >= 0 {
            current.saturating_add(delta as u128)
        } else {
            let required = (-delta) as u128;
            if current < required {
                return Err(StateError::InsufficientBalance {
                    required,
                    available: current,
                });
            }
            current - required
        };

        self.set_balance(address, new_balance)
    }

    /// Apply a nonce increment with validation
    pub fn apply_nonce_increment(
        &mut self,
        address: Address,
        expected_nonce: u64,
    ) -> Result<(), StateError> {
        let current = self.get_nonce(address)?;

        if current != expected_nonce {
            if expected_nonce > current + 1 {
                return Err(StateError::NonceGap {
                    expected: current + 1,
                    actual: expected_nonce,
                });
            }
            return Err(StateError::InvalidNonce {
                expected: current,
                actual: expected_nonce,
            });
        }

        self.increment_nonce(address)
    }

    // =========================================================================
    // PERSISTENCE METHODS
    // =========================================================================

    /// Serialize the trie state to bytes for persistence
    pub fn serialize(&self) -> Result<Vec<u8>, StateError> {
        let mut data = Vec::new();
        
        // Version byte
        data.push(1u8);
        
        // Root hash
        data.extend_from_slice(&self.root);
        
        // Account count
        let account_count = self.accounts.len() as u32;
        data.extend_from_slice(&account_count.to_le_bytes());
        
        // Serialize each account
        for (address, account) in &self.accounts {
            data.extend_from_slice(address);
            data.extend_from_slice(&account.balance.to_le_bytes());
            data.extend_from_slice(&account.nonce.to_le_bytes());
            data.extend_from_slice(&account.code_hash);
            data.extend_from_slice(&account.storage_root);
        }
        
        // Storage count
        let storage_count = self.storage.len() as u32;
        data.extend_from_slice(&storage_count.to_le_bytes());
        
        // Serialize storage
        for ((address, key), value) in &self.storage {
            data.extend_from_slice(address);
            data.extend_from_slice(key);
            data.extend_from_slice(value);
        }
        
        Ok(data)
    }

    /// Deserialize trie state from bytes
    pub fn deserialize(data: &[u8]) -> Result<Self, StateError> {
        if data.is_empty() {
            return Ok(Self::new());
        }
        
        let mut cursor = 0;
        
        // Version check
        if data[cursor] != 1 {
            return Err(StateError::DatabaseError("Unsupported trie version".to_string()));
        }
        cursor += 1;
        
        // Root hash
        let mut root = [0u8; 32];
        root.copy_from_slice(&data[cursor..cursor + 32]);
        cursor += 32;
        
        // Account count
        let account_count = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]) as usize;
        cursor += 4;
        
        // Deserialize accounts
        let mut accounts = HashMap::with_capacity(account_count);
        let mut storage_counts = HashMap::new();
        
        for _ in 0..account_count {
            let mut address = [0u8; 20];
            address.copy_from_slice(&data[cursor..cursor + 20]);
            cursor += 20;
            
            let balance = u128::from_le_bytes([
                data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3],
                data[cursor + 4], data[cursor + 5], data[cursor + 6], data[cursor + 7],
                data[cursor + 8], data[cursor + 9], data[cursor + 10], data[cursor + 11],
                data[cursor + 12], data[cursor + 13], data[cursor + 14], data[cursor + 15],
            ]);
            cursor += 16;
            
            let nonce = u64::from_le_bytes([
                data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3],
                data[cursor + 4], data[cursor + 5], data[cursor + 6], data[cursor + 7],
            ]);
            cursor += 8;
            
            let mut code_hash = [0u8; 32];
            code_hash.copy_from_slice(&data[cursor..cursor + 32]);
            cursor += 32;
            
            let mut storage_root = [0u8; 32];
            storage_root.copy_from_slice(&data[cursor..cursor + 32]);
            cursor += 32;
            
            accounts.insert(address, AccountState {
                balance,
                nonce,
                code_hash,
                storage_root,
            });
        }
        
        // Storage count
        let storage_count = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]) as usize;
        cursor += 4;
        
        // Deserialize storage
        let mut storage = HashMap::with_capacity(storage_count);
        
        for _ in 0..storage_count {
            let mut address = [0u8; 20];
            address.copy_from_slice(&data[cursor..cursor + 20]);
            cursor += 20;
            
            let mut key = [0u8; 32];
            key.copy_from_slice(&data[cursor..cursor + 32]);
            cursor += 32;
            
            let mut value = [0u8; 32];
            value.copy_from_slice(&data[cursor..cursor + 32]);
            cursor += 32;
            
            *storage_counts.entry(address).or_insert(0) += 1;
            storage.insert((address, key), value);
        }
        
        Ok(Self {
            root,
            accounts,
            storage,
            storage_counts,
            config: StateConfig::default(),
        })
    }

    /// Save state to a TrieDatabase
    pub fn save_to_db<D: crate::ports::TrieDatabase>(&self, db: &D) -> Result<(), StateError> {
        let data = self.serialize()?;
        let state_key = [0xFFu8; 32]; // Special key for full state
        db.put_node(state_key, data)
    }

    /// Load state from a TrieDatabase  
    pub fn load_from_db<D: crate::ports::TrieDatabase>(db: &D) -> Result<Self, StateError> {
        let state_key = [0xFFu8; 32]; // Special key for full state
        match db.get_node(&state_key)? {
            Some(data) => Self::deserialize(&data),
            None => Ok(Self::new()),
        }
    }
}

impl Default for PatriciaMerkleTrie {
    fn default() -> Self {
        Self::new()
    }
}

pub fn verify_proof(proof: &StateProof, address: &Address, root: &Hash) -> bool {
    // Must match the root
    if proof.state_root != *root {
        return false;
    }

    // Must be for the same address
    if proof.address != *address {
        return false;
    }

    // If account exists, proof nodes should not be empty
    if proof.account_state.is_some() && proof.proof_nodes.is_empty() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get_account() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = [0xAB; 20];

        let account = AccountState {
            balance: 1_000_000,
            nonce: 42,
            code_hash: [0; 32],
            storage_root: EMPTY_TRIE_ROOT,
        };

        trie.insert_account(address, &account).unwrap();
        let retrieved = trie.get_account(address).unwrap();

        assert_eq!(retrieved, Some(account));
    }

    #[test]
    fn test_deterministic_root() {
        let mut trie1 = PatriciaMerkleTrie::new();
        let mut trie2 = PatriciaMerkleTrie::new();

        let transitions = vec![
            ([1u8; 20], 100u128),
            ([2u8; 20], 200u128),
            ([3u8; 20], 300u128),
        ];

        for (addr, balance) in &transitions {
            trie1.set_balance(*addr, *balance).unwrap();
            trie2.set_balance(*addr, *balance).unwrap();
        }

        assert_eq!(trie1.root_hash(), trie2.root_hash());
    }

    #[test]
    fn test_balance_underflow_protection() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = [0xAB; 20];

        trie.set_balance(address, 100).unwrap();

        let result = trie.apply_balance_change(address, -101);
        assert!(matches!(
            result,
            Err(StateError::InsufficientBalance { .. })
        ));
    }

    #[test]
    fn test_nonce_monotonicity() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = [0xAB; 20];

        // Set initial nonce
        trie.insert_account(address, &AccountState::new(1000).with_nonce(5))
            .unwrap();

        // Valid increment
        let result = trie.apply_nonce_increment(address, 5);
        assert!(result.is_ok());
        assert_eq!(trie.get_nonce(address).unwrap(), 6);

        // Invalid: trying to use old nonce
        let result = trie.apply_nonce_increment(address, 5);
        assert!(matches!(result, Err(StateError::InvalidNonce { .. })));

        // Invalid: trying to skip nonce
        let result = trie.apply_nonce_increment(address, 10);
        assert!(matches!(result, Err(StateError::NonceGap { .. })));
    }

    #[test]
    fn test_storage_limit() {
        let config = StateConfig {
            max_storage_slots_per_contract: 3,
            ..Default::default()
        };
        let mut trie = PatriciaMerkleTrie::with_config(config);
        let contract = [0x42; 20];

        // Should succeed for first 3 slots
        for i in 0..3 {
            let mut key = [0u8; 32];
            key[0] = i;
            trie.set_storage(contract, key, [0xFF; 32]).unwrap();
        }

        // 4th slot should fail
        let result = trie.set_storage(contract, [0x03; 32], [0xFF; 32]);
        assert!(matches!(
            result,
            Err(StateError::StorageLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_proof_generation() {
        let mut trie = PatriciaMerkleTrie::new();
        let address = [0xCD; 20];

        let account = AccountState {
            balance: 500,
            nonce: 1,
            code_hash: [0; 32],
            storage_root: EMPTY_TRIE_ROOT,
        };

        trie.insert_account(address, &account).unwrap();

        let proof = trie.generate_proof(address).unwrap();
        let verified = verify_proof(&proof, &address, &trie.root_hash());

        assert!(verified);
    }
}
