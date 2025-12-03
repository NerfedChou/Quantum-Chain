use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];
pub type Address = [u8; 20];
pub type StorageKey = [u8; 32];
pub type StorageValue = [u8; 32];

pub const EMPTY_CODE_HASH: Hash = [0u8; 32];
pub const EMPTY_TRIE_ROOT: Hash = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6,
    0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0,
    0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];

/// Account state stored in the Patricia Merkle Trie
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    pub balance: u128,
    pub nonce: u64,
    pub code_hash: Hash,
    pub storage_root: Hash,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            balance: 0,
            nonce: 0,
            code_hash: EMPTY_CODE_HASH,
            storage_root: EMPTY_TRIE_ROOT,
        }
    }
}

impl AccountState {
    pub fn new(balance: u128) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }

    pub fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }
}

/// State transition for a single account
#[derive(Clone, Debug)]
pub struct AccountTransition {
    pub address: Address,
    pub balance_delta: i128,
    pub nonce_increment: u64,
    pub storage_changes: Vec<(StorageKey, Option<StorageValue>)>,
    pub code_change: Option<Vec<u8>>,
}

impl AccountTransition {
    pub fn transfer(address: Address, delta: i128) -> Self {
        Self {
            address,
            balance_delta: delta,
            nonce_increment: if delta < 0 { 1 } else { 0 },
            storage_changes: vec![],
            code_change: None,
        }
    }
}

/// Complete state transition for a block
#[derive(Clone, Debug)]
pub struct BlockStateTransition {
    pub block_hash: Hash,
    pub block_height: u64,
    pub account_transitions: Vec<AccountTransition>,
    pub previous_state_root: Hash,
}

/// Configuration for the state trie
#[derive(Clone, Debug)]
pub struct StateConfig {
    pub max_depth: usize,
    pub cache_size_mb: usize,
    pub enable_snapshots: bool,
    pub snapshot_interval: u64,
    pub pruning_depth: u64,
    pub max_storage_slots_per_contract: usize,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            max_depth: 64,
            cache_size_mb: 512,
            enable_snapshots: true,
            snapshot_interval: 128,
            pruning_depth: 1000,
            max_storage_slots_per_contract: 10_000,
        }
    }
}
