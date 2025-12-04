//! # Domain Entities for State Management
//!
//! Core data structures per SPEC-04 Section 2.1.
//!
//! ## Type Decisions
//!
//! - `balance: u128` - Sufficient for 340 undecillion units. U256 would require
//!   primitive-types dependency and complex arithmetic. u128 covers all practical
//!   blockchain use cases while maintaining simplicity.
//!
//! ## References
//!
//! - SPEC-04 Section 2.1: Core Entities
//! - Architecture.md Section 2.1: DDD principles

use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];
pub type Address = [u8; 20];
pub type StorageKey = [u8; 32];
pub type StorageValue = [u8; 32];

/// Empty code hash for externally owned accounts (EOAs).
/// Contracts have non-zero code_hash after deployment.
pub const EMPTY_CODE_HASH: Hash = [0u8; 32];

/// Keccak256 hash of an empty RLP-encoded trie.
/// This is the canonical empty trie root per Ethereum specification.
/// Value: keccak256(RLP("")) = 0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421
pub const EMPTY_TRIE_ROOT: Hash = [
    0x56, 0xe8, 0x1f, 0x17, 0x1b, 0xcc, 0x55, 0xa6, 0xff, 0x83, 0x45, 0xe6, 0x92, 0xc0, 0xf8, 0x6e,
    0x5b, 0x48, 0xe0, 0x1b, 0x99, 0x6c, 0xad, 0xc0, 0x01, 0x62, 0x2f, 0xb5, 0xe3, 0x63, 0xb4, 0x21,
];

/// Account state stored in the Patricia Merkle Trie.
///
/// Each account in the blockchain has this state structure. The account
/// is identified by its 20-byte address (derived from public key).
///
/// ## Fields
///
/// - `balance`: Token balance in base units (wei equivalent)
/// - `nonce`: Transaction count, prevents replay attacks (INVARIANT-2)
/// - `code_hash`: Hash of contract bytecode (EMPTY_CODE_HASH for EOAs)
/// - `storage_root`: Root of account's storage trie (EMPTY_TRIE_ROOT if empty)
///
/// ## Serialization
///
/// RLP-encoded as: [nonce, balance, storage_root, code_hash]
/// This ordering matches Ethereum's account encoding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountState {
    /// Account balance in base units. u128 supports up to 340 undecillion units.
    pub balance: u128,
    /// Transaction nonce. Increments by exactly 1 per processed transaction.
    pub nonce: u64,
    /// Keccak256 hash of contract code. EMPTY_CODE_HASH for non-contract accounts.
    pub code_hash: Hash,
    /// Root hash of the account's storage Patricia Merkle Trie.
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
    /// Create a new account with the specified balance.
    pub fn new(balance: u128) -> Self {
        Self {
            balance,
            ..Default::default()
        }
    }

    /// Builder method to set nonce.
    pub fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }

    /// RLP-encode this account state for hashing.
    ///
    /// Encoding order: [nonce, balance, storage_root, code_hash]
    /// This matches Ethereum's account RLP encoding.
    pub fn rlp_encode(&self) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(128);

        // Encode nonce (variable length integer)
        rlp_encode_u64(&mut encoded, self.nonce);

        // Encode balance (variable length integer, up to 16 bytes)
        rlp_encode_u128(&mut encoded, self.balance);

        // Encode storage_root (32 bytes)
        rlp_encode_bytes(&mut encoded, &self.storage_root);

        // Encode code_hash (32 bytes)
        rlp_encode_bytes(&mut encoded, &self.code_hash);

        // Wrap in list
        rlp_encode_list(encoded)
    }
}

/// RLP-encode a u64 value.
fn rlp_encode_u64(out: &mut Vec<u8>, value: u64) {
    if value == 0 {
        out.push(0x80); // Empty string
    } else if value < 128 {
        out.push(value as u8);
    } else {
        let bytes = value.to_be_bytes();
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(8);
        let len = 8 - start;
        out.push(0x80 + len as u8);
        out.extend_from_slice(&bytes[start..]);
    }
}

/// RLP-encode a u128 value.
fn rlp_encode_u128(out: &mut Vec<u8>, value: u128) {
    if value == 0 {
        out.push(0x80);
    } else if value < 128 {
        out.push(value as u8);
    } else {
        let bytes = value.to_be_bytes();
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(16);
        let len = 16 - start;
        out.push(0x80 + len as u8);
        out.extend_from_slice(&bytes[start..]);
    }
}

/// RLP-encode a byte slice.
fn rlp_encode_bytes(out: &mut Vec<u8>, data: &[u8]) {
    if data.len() == 1 && data[0] < 128 {
        out.push(data[0]);
    } else if data.len() < 56 {
        out.push(0x80 + data.len() as u8);
        out.extend_from_slice(data);
    } else {
        let len_bytes = data.len().to_be_bytes();
        let len_start = len_bytes.iter().position(|&b| b != 0).unwrap_or(8);
        let len_len = 8 - len_start;
        out.push(0xb7 + len_len as u8);
        out.extend_from_slice(&len_bytes[len_start..]);
        out.extend_from_slice(data);
    }
}

/// Wrap encoded items in an RLP list.
fn rlp_encode_list(items: Vec<u8>) -> Vec<u8> {
    let mut result = Vec::with_capacity(items.len() + 9);
    if items.len() < 56 {
        result.push(0xc0 + items.len() as u8);
    } else {
        let len_bytes = items.len().to_be_bytes();
        let len_start = len_bytes.iter().position(|&b| b != 0).unwrap_or(8);
        let len_len = 8 - len_start;
        result.push(0xf7 + len_len as u8);
        result.extend_from_slice(&len_bytes[len_start..]);
    }
    result.extend(items);
    result
}

/// State transition for a single account within a block.
///
/// Represents changes to apply to an account's state during block processing.
/// Used by the choreography handler when processing BlockValidated events.
///
/// ## INVARIANT-1 Enforcement
///
/// `balance_delta` can be negative (spending), but the resulting balance
/// must remain non-negative. This is enforced during application.
///
/// ## INVARIANT-2 Enforcement
///
/// `nonce_increment` must be exactly 1 for processed transactions, 0 otherwise.
#[derive(Clone, Debug)]
pub struct AccountTransition {
    /// Target account address.
    pub address: Address,
    /// Balance change (positive = credit, negative = debit).
    pub balance_delta: i128,
    /// Nonce increment (must be 0 or 1 per INVARIANT-2).
    pub nonce_increment: u64,
    /// Storage slot changes. None value = deletion.
    pub storage_changes: Vec<(StorageKey, Option<StorageValue>)>,
    /// New contract code (for contract deployment).
    pub code_change: Option<Vec<u8>>,
}

impl AccountTransition {
    /// Create a simple transfer transition.
    ///
    /// If `delta < 0`, this is a send operation and nonce increments.
    /// If `delta >= 0`, this is a receive operation and nonce stays same.
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

/// Complete state transition for a block.
///
/// Contains all account transitions that result from processing
/// a validated block. Used to batch-apply state changes atomically.
///
/// ## INVARIANT-5: Atomic Transitions
///
/// All transitions in a BlockStateTransition are applied atomically.
/// Either all succeed or none are applied (all-or-nothing semantics).
#[derive(Clone, Debug)]
pub struct BlockStateTransition {
    /// Hash of the block being processed.
    pub block_hash: Hash,
    /// Height of the block being processed.
    pub block_height: u64,
    /// All account transitions in this block.
    pub account_transitions: Vec<AccountTransition>,
    /// State root before applying this block.
    pub previous_state_root: Hash,
}

/// Configuration for the Patricia Merkle Trie.
///
/// Controls memory usage, caching behavior, and DoS protection limits.
#[derive(Clone, Debug)]
pub struct StateConfig {
    /// Maximum trie depth. Limits path length to prevent DoS.
    /// 64 is sufficient for 256-bit keys (64 nibbles).
    pub max_depth: usize,
    /// Size of in-memory node cache in megabytes.
    pub cache_size_mb: usize,
    /// Enable periodic state snapshots for fast sync.
    pub enable_snapshots: bool,
    /// Create snapshot every N blocks.
    pub snapshot_interval: u64,
    /// Keep state for last N blocks (older states pruned).
    pub pruning_depth: u64,
    /// Maximum storage slots per contract (DoS protection).
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
