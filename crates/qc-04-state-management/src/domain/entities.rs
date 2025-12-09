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

// =============================================================================
// SECURITY HARDENING CONSTANTS
// =============================================================================

/// Maximum proof depth for state proofs (anti-DoS).
///
/// 20-byte address = 40 nibbles, but with extensions/branches
/// actual depth can vary. 64 provides safety margin.
pub const MAX_PROOF_DEPTH: usize = 64;

/// Domain byte for leaf node hashing (anti-second-preimage).
/// Prevents leaf nodes from masquerading as internal nodes.
pub const LEAF_DOMAIN: u8 = 0x00;

/// Domain byte for extension node hashing.
pub const EXTENSION_DOMAIN: u8 = 0x01;

/// Domain byte for branch node hashing.
pub const BRANCH_DOMAIN: u8 = 0x02;

/// Maximum cached accounts in LRU cache.
pub const MAX_CACHED_ACCOUNTS: usize = 10_000;

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

// =============================================================================
// SECURE PATH HASHING (Anti-Grinding / Anti-Deep-Trie)
// =============================================================================

use sha3::{Digest, Keccak256};

/// Hash an address to get secure trie path.
///
/// ## Security: Anti-Grinding Attack
///
/// Attackers cannot generate addresses with specific prefixes that
/// would create deep trie paths. Keccak256 uniformly distributes keys.
///
/// ## Breaking Change
///
/// This MUST be implemented before Genesis. Changing after would
/// invalidate all state roots.
pub fn hash_path(address: &Address) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(address);
    hasher.finalize().into()
}

/// Hash a leaf node with domain separation.
///
/// LeafHash = Keccak256(0x00 || RLP(data))
pub fn hash_leaf(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(&[LEAF_DOMAIN]);
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash an extension node with domain separation.
///
/// ExtensionHash = Keccak256(0x01 || RLP(data))
pub fn hash_extension(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(&[EXTENSION_DOMAIN]);
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash a branch node with domain separation.
///
/// BranchHash = Keccak256(0x02 || RLP(data))
pub fn hash_branch(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(&[BRANCH_DOMAIN]);
    hasher.update(data);
    hasher.finalize().into()
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_separation_constants() {
        // All domain bytes must be unique
        assert_ne!(LEAF_DOMAIN, EXTENSION_DOMAIN);
        assert_ne!(LEAF_DOMAIN, BRANCH_DOMAIN);
        assert_ne!(EXTENSION_DOMAIN, BRANCH_DOMAIN);
        
        // Standard values
        assert_eq!(LEAF_DOMAIN, 0x00);
        assert_eq!(EXTENSION_DOMAIN, 0x01);
        assert_eq!(BRANCH_DOMAIN, 0x02);
    }

    #[test]
    fn test_max_proof_depth() {
        // 64 supports 32-byte keys (64 nibbles)
        assert_eq!(MAX_PROOF_DEPTH, 64);
    }

    #[test]
    fn test_hash_path_deterministic() {
        let address = [0x01; 20];
        let hash1 = hash_path(&address);
        let hash2 = hash_path(&address);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_path_different_addresses() {
        let addr1 = [0x01; 20];
        let addr2 = [0x02; 20];
        assert_ne!(hash_path(&addr1), hash_path(&addr2));
    }

    #[test]
    fn test_hash_path_uniform_distribution() {
        // Similar addresses should produce very different hashes
        let mut addr1 = [0x00; 20];
        let mut addr2 = [0x00; 20];
        addr1[19] = 0x01;
        addr2[19] = 0x02;
        
        let h1 = hash_path(&addr1);
        let h2 = hash_path(&addr2);
        
        // First bytes should differ (high probability with good hash)
        // This prevents prefix grinding attacks
        assert_ne!(h1[0], h2[0]);
    }

    #[test]
    fn test_domain_separation_hashes() {
        let data = b"test_node_data";
        
        let leaf = hash_leaf(data);
        let ext = hash_extension(data);
        let branch = hash_branch(data);
        
        // Same data, different hashes due to domain separation
        assert_ne!(leaf, ext);
        assert_ne!(leaf, branch);
        assert_ne!(ext, branch);
    }

    #[test]
    fn test_account_state_default() {
        let state = AccountState::default();
        assert_eq!(state.balance, 0);
        assert_eq!(state.nonce, 0);
        assert_eq!(state.code_hash, EMPTY_CODE_HASH);
        assert_eq!(state.storage_root, EMPTY_TRIE_ROOT);
    }
}
