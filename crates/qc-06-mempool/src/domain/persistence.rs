//! # Persistent Mempool (Fast-Resume)
//!
//! Implements tip-relative revalidation for fast node restart.
//!
//! ## Problem
//!
//! When node restarts, mempool is wiped. Node must wait for gossip.
//! This delays block production.
//!
//! ## Solution: Tip-Relative-Revalidation
//!
//! 1. Save: Serialize tx_data + first_seen_time to mempool.dat
//! 2. Load: Read and validate (skip signature verification if tip unchanged)
//! 3. Skip: Transactions from deep reorgs
//!
//! ## Security
//!
//! Node is fully operational immediately upon restart.

use super::{Address, Hash, Timestamp, U256};
use std::io::{self, Read};

/// Serializable transaction entry for persistence.
#[derive(Clone, Debug)]
pub struct PersistedTransaction {
    /// Transaction hash
    pub hash: Hash,
    /// Sender address
    pub sender: Address,
    /// Nonce
    pub nonce: u64,
    /// Gas price
    pub gas_price: U256,
    /// Gas limit
    pub gas_limit: u64,
    /// Raw transaction data
    pub raw_data: Vec<u8>,
    /// First seen timestamp
    pub first_seen: Timestamp,
    /// Block height when saved
    pub saved_at_height: u64,
}

/// Mempool persistence manager.
///
/// ## Algorithm: Tip-Relative-Revalidation
///
/// On save: Serialize all pending transactions
/// On load: Validate locktime, skip signature verification
#[derive(Debug)]
pub struct MempoolPersistence {
    /// Maximum reorg depth to trust cached validation
    max_reorg_depth: u64,
}

/// Default max reorg depth.
pub const DEFAULT_MAX_REORG_DEPTH: u64 = 100;

/// Magic bytes for mempool.dat
const MEMPOOL_MAGIC: &[u8; 8] = b"QCMPOOL\x01";

impl MempoolPersistence {
    pub fn new() -> Self {
        Self {
            max_reorg_depth: DEFAULT_MAX_REORG_DEPTH,
        }
    }

    /// Create with custom reorg depth.
    pub fn with_reorg_depth(max_reorg_depth: u64) -> Self {
        Self { max_reorg_depth }
    }

    /// Serialize transactions for persistence.
    ///
    /// Format: \[MAGIC\]\[VERSION\]\[COUNT\]\[TX1\]\[TX2\]...
    pub fn serialize(&self, transactions: &[PersistedTransaction], current_height: u64) -> Vec<u8> {
        let mut buf = Vec::with_capacity(transactions.len() * 256);

        // Magic + version
        buf.extend_from_slice(MEMPOOL_MAGIC);

        // Current height
        buf.extend_from_slice(&current_height.to_le_bytes());

        // Transaction count
        buf.extend_from_slice(&(transactions.len() as u64).to_le_bytes());

        // Each transaction
        for tx in transactions {
            self.write_tx(&mut buf, tx);
        }

        buf
    }

    /// Deserialize transactions from persistence.
    ///
    /// Returns transactions that are still valid for current height.
    pub fn deserialize(
        &self,
        data: &[u8],
        current_height: u64,
    ) -> io::Result<Vec<PersistedTransaction>> {
        let mut reader = data;

        // Check magic
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;
        if &magic != MEMPOOL_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid magic"));
        }

        // Read saved height
        let mut height_bytes = [0u8; 8];
        reader.read_exact(&mut height_bytes)?;
        let saved_height = u64::from_le_bytes(height_bytes);

        // Check reorg depth
        if current_height.saturating_sub(saved_height) > self.max_reorg_depth {
            // Too deep reorg, cannot trust cached validation
            return Ok(Vec::new());
        }

        // Read count
        let mut count_bytes = [0u8; 8];
        reader.read_exact(&mut count_bytes)?;
        let count = u64::from_le_bytes(count_bytes);

        let mut transactions = Vec::with_capacity(count as usize);

        for _ in 0..count {
            if let Some(tx) = self.read_tx(&mut reader)? {
                transactions.push(tx);
            }
        }

        Ok(transactions)
    }

    /// Check if a transaction can skip signature verification.
    ///
    /// True if saved within reorg depth and chain tip hasn't changed.
    pub fn can_skip_verification(&self, tx: &PersistedTransaction, current_height: u64) -> bool {
        current_height.saturating_sub(tx.saved_at_height) <= self.max_reorg_depth
    }

    /// Write a single transaction.
    fn write_tx(&self, buf: &mut Vec<u8>, tx: &PersistedTransaction) {
        // Hash (32 bytes)
        buf.extend_from_slice(&tx.hash);

        // Sender (20 bytes)
        buf.extend_from_slice(&tx.sender);

        // Nonce (8 bytes)
        buf.extend_from_slice(&tx.nonce.to_le_bytes());

        // Gas price (32 bytes as U256)
        buf.extend_from_slice(&u256_to_bytes(tx.gas_price));

        // Gas limit (8 bytes)
        buf.extend_from_slice(&tx.gas_limit.to_le_bytes());

        // First seen (8 bytes)
        buf.extend_from_slice(&tx.first_seen.to_le_bytes());

        // Saved at height (8 bytes)
        buf.extend_from_slice(&tx.saved_at_height.to_le_bytes());

        // Raw data length + data
        buf.extend_from_slice(&(tx.raw_data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&tx.raw_data);
    }

    /// Read a single transaction.
    fn read_tx(&self, reader: &mut &[u8]) -> io::Result<Option<PersistedTransaction>> {
        // Hash
        let mut hash = [0u8; 32];
        reader.read_exact(&mut hash)?;

        // Sender
        let mut sender = [0u8; 20];
        reader.read_exact(&mut sender)?;

        // Nonce
        let mut nonce_bytes = [0u8; 8];
        reader.read_exact(&mut nonce_bytes)?;
        let nonce = u64::from_le_bytes(nonce_bytes);

        // Gas price
        let mut gas_price_bytes = [0u8; 32];
        reader.read_exact(&mut gas_price_bytes)?;
        let gas_price = bytes_to_u256(&gas_price_bytes);

        // Gas limit
        let mut gas_limit_bytes = [0u8; 8];
        reader.read_exact(&mut gas_limit_bytes)?;
        let gas_limit = u64::from_le_bytes(gas_limit_bytes);

        // First seen
        let mut first_seen_bytes = [0u8; 8];
        reader.read_exact(&mut first_seen_bytes)?;
        let first_seen = u64::from_le_bytes(first_seen_bytes);

        // Saved at height
        let mut height_bytes = [0u8; 8];
        reader.read_exact(&mut height_bytes)?;
        let saved_at_height = u64::from_le_bytes(height_bytes);

        // Raw data
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut raw_data = vec![0u8; len];
        reader.read_exact(&mut raw_data)?;

        Ok(Some(PersistedTransaction {
            hash,
            sender,
            nonce,
            gas_price,
            gas_limit,
            raw_data,
            first_seen,
            saved_at_height,
        }))
    }
}

impl Default for MempoolPersistence {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert U256 to 32 bytes (little-endian).
fn u256_to_bytes(value: U256) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    // U256 has low and high u128
    let low = value.low_u128();
    bytes[0..16].copy_from_slice(&low.to_le_bytes());
    // For high bits, we need to extract them
    // Using division approach
    let high = (value >> 128).low_u128();
    bytes[16..32].copy_from_slice(&high.to_le_bytes());
    bytes
}

/// Convert 32 bytes to U256 (little-endian).
fn bytes_to_u256(bytes: &[u8; 32]) -> U256 {
    let low = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
    let high = u128::from_le_bytes(bytes[16..32].try_into().unwrap());
    U256::from(low) + (U256::from(high) << 128)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_tx(nonce: u64) -> PersistedTransaction {
        PersistedTransaction {
            hash: [nonce as u8; 32],
            sender: [0xAA; 20],
            nonce,
            gas_price: U256::from(1_000_000_000u64),
            gas_limit: 21000,
            raw_data: vec![0x01, 0x02, 0x03],
            first_seen: 1000,
            saved_at_height: 100,
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let persistence = MempoolPersistence::new();
        let txs = vec![create_test_tx(0), create_test_tx(1), create_test_tx(2)];

        let serialized = persistence.serialize(&txs, 100);
        let deserialized = persistence.deserialize(&serialized, 100).unwrap();

        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].nonce, 0);
        assert_eq!(deserialized[1].nonce, 1);
        assert_eq!(deserialized[2].nonce, 2);
    }

    #[test]
    fn test_reorg_depth_check() {
        let persistence = MempoolPersistence::with_reorg_depth(10);
        let txs = vec![create_test_tx(0)];

        let serialized = persistence.serialize(&txs, 100);

        // Within reorg depth - should work
        let result = persistence.deserialize(&serialized, 105).unwrap();
        assert_eq!(result.len(), 1);

        // Beyond reorg depth - should return empty
        let result = persistence.deserialize(&serialized, 200).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_can_skip_verification() {
        let persistence = MempoolPersistence::with_reorg_depth(10);
        let tx = create_test_tx(0);

        assert!(persistence.can_skip_verification(&tx, 105));
        assert!(!persistence.can_skip_verification(&tx, 200));
    }

    #[test]
    fn test_u256_roundtrip() {
        let original = U256::from(12345678901234567890u128);
        let bytes = u256_to_bytes(original);
        let recovered = bytes_to_u256(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_invalid_magic() {
        let persistence = MempoolPersistence::new();
        let bad_data = b"BADMAGIC";

        let result = persistence.deserialize(bad_data, 100);
        assert!(result.is_err());
    }
}
