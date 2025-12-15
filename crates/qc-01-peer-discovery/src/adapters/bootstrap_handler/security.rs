//! Proof of Work validation.

use crate::domain::NodeId;
use sha2::{Digest, Sha256};

/// Proof of Work wrapper for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofOfWork([u8; 32]);

impl ProofOfWork {
    /// Create a new proof of work from bytes.
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Validate proof-of-work for anti-Sybil protection.
    ///
    /// # Security (Hardened)
    ///
    /// PoW must satisfy: SHA256(node_id || proof_of_work) has N+ leading zero bits.
    /// This binds the proof to the identity. Production uses 24 bits (~16M attempts).
    pub fn validate(&self, node_id: &NodeId, difficulty: u32) -> bool {
        // Compute H(node_id || proof_of_work) and verify difficulty
        self.verify_binding(node_id, difficulty)
    }

    /// Verify that SHA256(node_id || nonce) has required leading zeros.
    fn verify_binding(&self, node_id: &NodeId, required_zeros: u32) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(node_id.as_bytes());
        hasher.update(self.0);
        let result = hasher.finalize();

        Self::count_leading_zero_bits(&result) >= required_zeros
    }

    /// Count leading zero bits in a byte slice.
    fn count_leading_zero_bits(bytes: &[u8]) -> u32 {
        let mut count = 0u32;
        for byte in bytes {
            if *byte == 0 {
                count += 8;
            } else {
                count += byte.leading_zeros();
                break;
            }
        }
        count
    }
}

impl From<[u8; 32]> for ProofOfWork {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Generate a secure correlation ID for request/response matching.
pub fn generate_correlation_id() -> [u8; 16] {
    // Use UUID v4 for correlation IDs (cryptographically random)
    let uuid = uuid::Uuid::new_v4();
    *uuid.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_leading_zero_bits() {
        // 0 zero bits
        assert_eq!(ProofOfWork::count_leading_zero_bits(&[0xFF]), 0);

        // 8 zero bits (1 byte)
        assert_eq!(ProofOfWork::count_leading_zero_bits(&[0x00, 0xFF]), 8);

        // 12 zero bits (1 byte + 4 bits from 0x0F which is 0000_1111)
        assert_eq!(ProofOfWork::count_leading_zero_bits(&[0x00, 0x0F]), 12);

        // 16 zero bits (2 bytes)
        assert_eq!(
            ProofOfWork::count_leading_zero_bits(&[0x00, 0x00, 0xFF]),
            16
        );

        // 24 zero bits (3 bytes + 0x80 = 1000_0000 has 0 leading zeros in that byte)
        assert_eq!(
            ProofOfWork::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x80]),
            24
        );

        // 25 zero bits (3 bytes + 0x40 = 0100_0000 has 1 leading zero)
        assert_eq!(
            ProofOfWork::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x40]),
            25
        );

        // 31 zero bits (3 bytes + 0x01 = 0000_0001 has 7 leading zeros)
        assert_eq!(
            ProofOfWork::count_leading_zero_bits(&[0x00, 0x00, 0x00, 0x01]),
            31
        );
    }

    #[test]
    fn test_verify_binding() {
        let node_id = NodeId::new([1u8; 32]);

        // Find a valid nonce that produces 8 leading zero bits (for quick test)
        let mut nonce = [0u8; 32];
        let mut found = false;
        for i in 0..100_000u32 {
            nonce[0..4].copy_from_slice(&i.to_le_bytes());
            let pow = ProofOfWork::new(nonce);
            if pow.validate(&node_id, 8) {
                found = true;
                break;
            }
        }
        assert!(found, "Should find valid 8-bit PoW within 100K attempts");
    }
}
