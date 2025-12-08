//! # Value Objects
//!
//! Immutable domain primitives for smart contract execution.
//! These types represent concepts that are defined by their value, not identity.

use serde::{Deserialize, Serialize};
use std::fmt;

// Re-export U256 from primitive-types for 256-bit arithmetic
pub use primitive_types::U256;

// =============================================================================
// ADDRESS (20 bytes)
// =============================================================================

/// A 20-byte Ethereum-style address.
///
/// Per IPC-MATRIX.md, all address fields use `[u8; 20]`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Address(pub [u8; 20]);

impl Address {
    /// The zero address (0x0000...0000).
    pub const ZERO: Self = Self([0u8; 20]);

    /// Creates an address from a 20-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }

    /// Creates an address from a slice. Returns None if wrong length.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() == 20 {
            let mut bytes = [0u8; 20];
            bytes.copy_from_slice(slice);
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Returns the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    /// Returns true if this is the zero address.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 20]
    }

    /// Checks if this address is a precompiled contract (0x01-0x09).
    #[must_use]
    pub fn is_precompile(&self) -> bool {
        // First 19 bytes must be zero
        if self.0[..19] != [0u8; 19] {
            return false;
        }
        // Last byte must be 1-9
        (1..=9).contains(&self.0[19])
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in &self.0[..4] {
            write!(f, "{byte:02x}")?;
        }
        write!(f, "...")?;
        for byte in &self.0[18..] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl From<[u8; 20]> for Address {
    fn from(bytes: [u8; 20]) -> Self {
        Self(bytes)
    }
}

impl From<Address> for [u8; 20] {
    fn from(addr: Address) -> Self {
        addr.0
    }
}

// =============================================================================
// HASH (32 bytes)
// =============================================================================

/// A 32-byte hash (e.g., Keccak-256).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    /// The zero hash.
    pub const ZERO: Self = Self([0u8; 32]);

    /// Creates a hash from a 32-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Creates a hash from a slice. Returns None if wrong length.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() == 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(slice);
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Returns the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns true if this is the zero hash.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for byte in &self.0[..4] {
            write!(f, "{byte:02x}")?;
        }
        write!(f, "...")?;
        for byte in &self.0[28..] {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<Hash> for [u8; 32] {
    fn from(hash: Hash) -> Self {
        hash.0
    }
}

// =============================================================================
// STORAGE KEY & VALUE (32 bytes each)
// =============================================================================

/// A 32-byte storage key.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct StorageKey(pub [u8; 32]);

impl StorageKey {
    /// The zero key.
    pub const ZERO: Self = Self([0u8; 32]);

    /// Creates a storage key from a 32-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Creates a storage key from a U256.
    #[must_use]
    pub fn from_u256(value: U256) -> Self {
        let mut bytes = [0u8; 32];
        value.to_big_endian(&mut bytes);
        Self(bytes)
    }

    /// Returns the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for StorageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StorageKey(0x")?;
        for byte in &self.0[..4] {
            write!(f, "{byte:02x}")?;
        }
        write!(f, "...)")
    }
}

impl From<[u8; 32]> for StorageKey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<U256> for StorageKey {
    fn from(value: U256) -> Self {
        Self::from_u256(value)
    }
}

/// A 32-byte storage value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct StorageValue(pub [u8; 32]);

impl StorageValue {
    /// The zero value.
    pub const ZERO: Self = Self([0u8; 32]);

    /// Creates a storage value from a 32-byte array.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Creates a storage value from a U256.
    #[must_use]
    pub fn from_u256(value: U256) -> Self {
        let mut bytes = [0u8; 32];
        value.to_big_endian(&mut bytes);
        Self(bytes)
    }

    /// Converts to U256.
    #[must_use]
    pub fn to_u256(&self) -> U256 {
        U256::from_big_endian(&self.0)
    }

    /// Returns the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns true if this is the zero value.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0 == [0u8; 32]
    }
}

impl fmt::Debug for StorageValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StorageValue({})", self.to_u256())
    }
}

impl From<[u8; 32]> for StorageValue {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<U256> for StorageValue {
    fn from(value: U256) -> Self {
        Self::from_u256(value)
    }
}

// =============================================================================
// BYTES (variable length)
// =============================================================================

/// Variable-length byte vector for calldata, return data, and code.
#[derive(Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    /// Creates an empty Bytes.
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates Bytes from a vector.
    #[must_use]
    pub fn from_vec(vec: Vec<u8>) -> Self {
        Self(vec)
    }

    /// Creates Bytes from a slice.
    #[must_use]
    pub fn from_slice(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }

    /// Returns the underlying vector.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }

    /// Returns a reference to the underlying slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Returns the length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() <= 8 {
            write!(f, "0x")?;
            for byte in &self.0 {
                write!(f, "{byte:02x}")?;
            }
        } else {
            write!(f, "0x")?;
            for byte in &self.0[..4] {
                write!(f, "{byte:02x}")?;
            }
            write!(f, "..({} bytes)", self.0.len())?;
        }
        Ok(())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Self {
        Self(vec)
    }
}

impl From<&[u8]> for Bytes {
    fn from(slice: &[u8]) -> Self {
        Self(slice.to_vec())
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// =============================================================================
// GAS COUNTER
// =============================================================================

/// Tracks gas consumption during execution.
///
/// ## Invariants
/// - `used <= limit` at all times
/// - Operations that exceed limit return `OutOfGas` error
#[derive(Clone, Copy, Debug, Default)]
pub struct GasCounter {
    /// Gas limit for this execution context.
    limit: u64,
    /// Gas consumed so far.
    used: u64,
    /// Gas refund accumulated (for SSTORE clears).
    refund: u64,
}

impl GasCounter {
    /// Creates a new gas counter with the given limit.
    #[must_use]
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            used: 0,
            refund: 0,
        }
    }

    /// Returns the gas limit.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// Returns gas used so far.
    #[must_use]
    pub const fn used(&self) -> u64 {
        self.used
    }

    /// Returns remaining gas.
    #[must_use]
    pub const fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Returns accumulated refund.
    #[must_use]
    pub const fn refund(&self) -> u64 {
        self.refund
    }

    /// Consumes gas. Returns false if insufficient gas.
    pub fn consume(&mut self, amount: u64) -> bool {
        if self.used.saturating_add(amount) > self.limit {
            false
        } else {
            self.used = self.used.saturating_add(amount);
            true
        }
    }

    /// Adds to refund counter.
    pub fn add_refund(&mut self, amount: u64) {
        self.refund = self.refund.saturating_add(amount);
    }

    /// Subtracts from refund counter.
    pub fn sub_refund(&mut self, amount: u64) {
        self.refund = self.refund.saturating_sub(amount);
    }

    /// Returns effective gas used after refund (capped at 50% per EIP-3529).
    #[must_use]
    pub fn effective_gas_used(&self) -> u64 {
        let max_refund = self.used / 2; // Cap at 50%
        let actual_refund = self.refund.min(max_refund);
        self.used.saturating_sub(actual_refund)
    }
}

// =============================================================================
// ECDSA SIGNATURE
// =============================================================================

/// ECDSA signature (r, s, v) for ecrecover precompile.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct EcdsaSignature {
    /// r component (32 bytes).
    pub r: [u8; 32],
    /// s component (32 bytes).
    pub s: [u8; 32],
    /// Recovery id (0 or 1, or 27/28 in legacy format).
    pub v: u8,
}

impl EcdsaSignature {
    /// Creates a new signature.
    #[must_use]
    pub const fn new(r: [u8; 32], s: [u8; 32], v: u8) -> Self {
        Self { r, s, v }
    }

    /// Normalizes v to 0 or 1.
    #[must_use]
    pub const fn normalized_v(&self) -> u8 {
        if self.v >= 27 {
            self.v - 27
        } else {
            self.v
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_zero() {
        assert!(Address::ZERO.is_zero());
        assert!(!Address::new([1u8; 20]).is_zero());
    }

    #[test]
    fn test_address_precompile() {
        let mut addr = [0u8; 20];
        addr[19] = 1;
        assert!(Address::new(addr).is_precompile());

        addr[19] = 9;
        assert!(Address::new(addr).is_precompile());

        addr[19] = 10;
        assert!(!Address::new(addr).is_precompile());

        addr[19] = 0;
        assert!(!Address::new(addr).is_precompile());
    }

    #[test]
    fn test_gas_counter() {
        let mut gas = GasCounter::new(1000);
        assert_eq!(gas.remaining(), 1000);

        assert!(gas.consume(500));
        assert_eq!(gas.used(), 500);
        assert_eq!(gas.remaining(), 500);

        assert!(!gas.consume(600)); // Would exceed limit
        assert_eq!(gas.used(), 500); // Unchanged

        gas.add_refund(100);
        assert_eq!(gas.refund(), 100);

        // Effective gas: 500 - min(100, 250) = 400
        assert_eq!(gas.effective_gas_used(), 400);
    }

    #[test]
    fn test_gas_refund_cap() {
        let mut gas = GasCounter::new(1000);
        gas.consume(800);
        gas.add_refund(500); // More than 50% of used

        // Capped at 50% of 800 = 400
        assert_eq!(gas.effective_gas_used(), 400);
    }

    #[test]
    fn test_storage_value_u256_conversion() {
        let value = U256::from(42);
        let storage = StorageValue::from_u256(value);
        assert_eq!(storage.to_u256(), value);
    }
}
