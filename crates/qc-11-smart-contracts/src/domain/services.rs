//! # Domain Services
//!
//! Pure business logic functions for smart contract execution.
//! These functions are deterministic and have no side effects.
//!
//! ## Architecture Compliance (Architecture.md v2.3)
//!
//! - NO I/O operations
//! - NO async code
//! - NO external dependencies
//! - Pure functions only

use crate::domain::value_objects::{Address, Hash};
use sha3::{Digest, Keccak256};

// =============================================================================
// CONTRACT ADDRESS COMPUTATION
// =============================================================================

/// Computes the contract address for CREATE opcode.
///
/// Address = keccak256(rlp(\[sender, nonce\]))\[12:\]
///
/// Per Ethereum Yellow Paper, section 7.
#[must_use]
pub fn compute_contract_address(sender: Address, nonce: u64) -> Address {
    // RLP encode [sender, nonce]
    let mut rlp_data = Vec::with_capacity(64);

    // RLP list header (will be updated)
    let mut content = Vec::with_capacity(32);

    // RLP encode address (20 bytes, 0x80 + 20 = 0x94)
    content.push(0x94);
    content.extend_from_slice(sender.as_bytes());

    // RLP encode nonce
    if nonce == 0 {
        content.push(0x80); // Empty byte string
    } else if nonce < 128 {
        content.push(nonce as u8);
    } else {
        // Encode as bytes
        let nonce_bytes = encode_nonce(nonce);
        content.push(0x80 + nonce_bytes.len() as u8);
        content.extend_from_slice(&nonce_bytes);
    }

    // RLP list header
    if content.len() < 56 {
        rlp_data.push(0xc0 + content.len() as u8);
    } else {
        let len_bytes = encode_length(content.len());
        rlp_data.push(0xf7 + len_bytes.len() as u8);
        rlp_data.extend_from_slice(&len_bytes);
    }
    rlp_data.extend_from_slice(&content);

    // Hash and take last 20 bytes
    let hash = Keccak256::digest(&rlp_data);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..32]);
    Address::new(addr)
}

/// Computes the contract address for CREATE2 opcode.
///
/// Address = keccak256(0xff ++ sender ++ salt ++ `keccak256(init_code)`)\[12:\]
///
/// Per EIP-1014.
#[must_use]
pub fn compute_contract_address_create2(sender: Address, salt: Hash, init_code: &[u8]) -> Address {
    let code_hash = Keccak256::digest(init_code);

    let mut data = Vec::with_capacity(85);
    data.push(0xff);
    data.extend_from_slice(sender.as_bytes());
    data.extend_from_slice(salt.as_bytes());
    data.extend_from_slice(&code_hash);

    let hash = Keccak256::digest(&data);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..32]);
    Address::new(addr)
}

/// Helper to encode nonce as big-endian bytes without leading zeros.
fn encode_nonce(nonce: u64) -> Vec<u8> {
    let bytes = nonce.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    bytes[start..].to_vec()
}

/// Helper to encode length for RLP.
fn encode_length(len: usize) -> Vec<u8> {
    let bytes = (len as u64).to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    bytes[start..].to_vec()
}

// =============================================================================
// GAS ESTIMATION
// =============================================================================

/// Estimates gas for a transaction.
///
/// This is a simplified estimation. Full estimation requires actual execution.
#[must_use]
pub fn estimate_base_gas(data: &[u8], is_contract_creation: bool) -> u64 {
    // Base transaction gas
    let base = if is_contract_creation {
        53_000 // CREATE transaction base
    } else {
        21_000 // Regular transaction base
    };

    // Data gas: 16 gas per non-zero byte, 4 gas per zero byte
    let data_gas: u64 = data
        .iter()
        .map(|&byte| if byte == 0 { 4u64 } else { 16u64 })
        .sum();

    base + data_gas
}

// =============================================================================
// KECCAK256 UTILITY
// =============================================================================

/// Computes keccak256 hash of data.
#[must_use]
pub fn keccak256(data: &[u8]) -> Hash {
    let hash = Keccak256::digest(data);
    Hash::new(hash.into())
}

/// Computes keccak256 of empty bytes (used for empty code hash).
#[must_use]
pub fn empty_code_hash() -> Hash {
    keccak256(&[])
}

// =============================================================================
// ADDRESS DERIVATION
// =============================================================================

/// Derives address from public key (compressed or uncompressed).
///
/// Address = `keccak256(public_key)`\[12:\]
///
/// Note: For ECDSA, the public key should be the uncompressed form (64 bytes)
/// without the 0x04 prefix.
#[must_use]
pub fn derive_address_from_pubkey(public_key: &[u8]) -> Address {
    let hash = Keccak256::digest(public_key);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..32]);
    Address::new(addr)
}

// =============================================================================
// PRECOMPILE ADDRESSES
// =============================================================================

/// Standard Ethereum precompile addresses.
pub mod precompiles {
    use super::Address;

    /// ecrecover (0x01)
    pub const ECRECOVER: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

    /// SHA256 (0x02)
    pub const SHA256: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);

    /// RIPEMD160 (0x03)
    pub const RIPEMD160: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3]);

    /// Identity / data copy (0x04)
    pub const IDENTITY: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4]);

    /// Modexp (0x05)
    pub const MODEXP: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5]);

    /// BN128 Add (0x06)
    pub const BN128_ADD: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6]);

    /// BN128 Mul (0x07)
    pub const BN128_MUL: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7]);

    /// BN128 Pairing (0x08)
    pub const BN128_PAIRING: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8]);

    /// Blake2f (0x09)
    pub const BLAKE2F: Address =
        Address([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9]);

    /// Returns the precompile address for a given number (1-9).
    #[must_use]
    pub fn from_number(n: u8) -> Option<Address> {
        if (1..=9).contains(&n) {
            let mut addr = [0u8; 20];
            addr[19] = n;
            Some(Address::new(addr))
        } else {
            None
        }
    }

    /// Checks if an address is a precompile.
    #[must_use]
    pub fn is_precompile(addr: &Address) -> bool {
        addr.is_precompile()
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_contract_address_nonce_zero() {
        // Test vector from Ethereum
        let sender = Address::new([
            0x6a, 0xc7, 0xea, 0x33, 0xf8, 0x83, 0x1e, 0xa9, 0xdd, 0xc2, 0x8e, 0xa9, 0x9d, 0xdc,
            0x3c, 0x4d, 0xdb, 0x70, 0x2c, 0x1c,
        ]);
        let addr = compute_contract_address(sender, 0);
        // The computed address should be deterministic
        assert!(!addr.is_zero());
    }

    #[test]
    fn test_compute_contract_address_nonce_one() {
        let sender = Address::new([1u8; 20]);
        let addr0 = compute_contract_address(sender, 0);
        let addr1 = compute_contract_address(sender, 1);

        // Different nonces should give different addresses
        assert_ne!(addr0, addr1);
    }

    #[test]
    fn test_compute_contract_address_deterministic() {
        let sender = Address::new([42u8; 20]);
        let nonce = 100;

        let addr1 = compute_contract_address(sender, nonce);
        let addr2 = compute_contract_address(sender, nonce);

        // Same inputs should give same output
        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_compute_contract_address_create2() {
        let sender = Address::new([1u8; 20]);
        let salt = Hash::new([0u8; 32]);
        let init_code = vec![0x60, 0x80, 0x60, 0x40]; // Sample bytecode

        let addr = compute_contract_address_create2(sender, salt, &init_code);
        assert!(!addr.is_zero());
    }

    #[test]
    fn test_create2_deterministic() {
        let sender = Address::new([1u8; 20]);
        let salt = Hash::new([42u8; 32]);
        let init_code = vec![0x00];

        let addr1 = compute_contract_address_create2(sender, salt, &init_code);
        let addr2 = compute_contract_address_create2(sender, salt, &init_code);

        assert_eq!(addr1, addr2);
    }

    #[test]
    fn test_create2_different_salt() {
        let sender = Address::new([1u8; 20]);
        let salt1 = Hash::new([1u8; 32]);
        let salt2 = Hash::new([2u8; 32]);
        let init_code = vec![0x00];

        let addr1 = compute_contract_address_create2(sender, salt1, &init_code);
        let addr2 = compute_contract_address_create2(sender, salt2, &init_code);

        assert_ne!(addr1, addr2);
    }

    #[test]
    fn test_estimate_base_gas_simple() {
        // Empty data, not contract creation
        let gas = estimate_base_gas(&[], false);
        assert_eq!(gas, 21_000);
    }

    #[test]
    fn test_estimate_base_gas_contract_creation() {
        let gas = estimate_base_gas(&[], true);
        assert_eq!(gas, 53_000);
    }

    #[test]
    fn test_estimate_base_gas_with_data() {
        // 10 non-zero bytes = 10 * 16 = 160
        // 5 zero bytes = 5 * 4 = 20
        // Total data gas = 180
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0, 0, 0, 0, 0];
        let gas = estimate_base_gas(&data, false);
        assert_eq!(gas, 21_000 + 160 + 20);
    }

    #[test]
    fn test_keccak256() {
        // Test vector: keccak256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let hash = keccak256(&[]);
        assert_eq!(hash.as_bytes()[0..4], [0xc5, 0xd2, 0x46, 0x01]);
    }

    #[test]
    fn test_empty_code_hash() {
        let hash = empty_code_hash();
        // This is the well-known empty code hash
        assert_eq!(hash.as_bytes()[0], 0xc5);
        assert_eq!(hash.as_bytes()[1], 0xd2);
    }

    #[test]
    fn test_precompile_addresses() {
        assert!(precompiles::ECRECOVER.is_precompile());
        assert!(precompiles::SHA256.is_precompile());
        assert!(precompiles::BLAKE2F.is_precompile());

        // Non-precompile
        assert!(!Address::new([1u8; 20]).is_precompile());
    }

    #[test]
    fn test_precompile_from_number() {
        let ecrecover = precompiles::from_number(1).unwrap();
        assert_eq!(ecrecover, precompiles::ECRECOVER);

        let blake2f = precompiles::from_number(9).unwrap();
        assert_eq!(blake2f, precompiles::BLAKE2F);

        assert!(precompiles::from_number(0).is_none());
        assert!(precompiles::from_number(10).is_none());
    }
}
