use super::{Address, StorageKey};

// =============================================================================
// NIBBLES: Half-byte path representation
// =============================================================================

/// Nibble path for trie traversal.
///
/// Addresses and keys are converted to nibbles (half-bytes, 0-15) for
/// traversal through the trie. A 20-byte address becomes 40 nibbles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Nibbles(pub Vec<u8>);

impl Nibbles {
    /// Create nibbles from a 20-byte address.
    pub fn from_address(addr: &Address) -> Self {
        let mut nibbles = Vec::with_capacity(40);
        for byte in addr {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
        Nibbles(nibbles)
    }

    /// Create nibbles from a 32-byte storage key.
    pub fn from_key(key: &StorageKey) -> Self {
        let mut nibbles = Vec::with_capacity(64);
        for byte in key {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
        Nibbles(nibbles)
    }

    /// Create nibbles from arbitrary bytes (used for hashed keys).
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut nibbles = Vec::with_capacity(bytes.len() * 2);
        for byte in bytes {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }
        Nibbles(nibbles)
    }

    /// Get a slice of nibbles starting at offset.
    pub fn slice(&self, start: usize) -> Self {
        Nibbles(self.0[start..].to_vec())
    }

    /// Get a range slice of nibbles.
    pub fn slice_range(&self, start: usize, end: usize) -> Self {
        Nibbles(self.0[start..end].to_vec())
    }

    /// Find common prefix length with another nibbles path.
    pub fn common_prefix_len(&self, other: &Nibbles) -> usize {
        self.0
            .iter()
            .zip(other.0.iter())
            .take_while(|(a, b)| a == b)
            .count()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get nibble at index.
    pub fn at(&self, index: usize) -> u8 {
        self.0[index]
    }

    /// Encode nibbles with hex-prefix for RLP encoding.
    ///
    /// Per Ethereum Yellow Paper:
    /// - First nibble encodes flags: 0=extension even, 1=extension odd, 2=leaf even, 3=leaf odd
    /// - If odd number of nibbles, first nibble is part of path
    pub fn encode_hex_prefix(&self, is_leaf: bool) -> Vec<u8> {
        let odd = self.len() % 2 == 1;
        let prefix = if is_leaf { 2 } else { 0 } + if odd { 1 } else { 0 };

        let mut result = Vec::with_capacity((self.len() + 2) / 2);

        if odd {
            result.push((prefix << 4) | self.0[0]);
            for chunk in self.0[1..].chunks(2) {
                result.push((chunk[0] << 4) | chunk.get(1).copied().unwrap_or(0));
            }
        } else {
            result.push(prefix << 4);
            for chunk in self.0.chunks(2) {
                result.push((chunk[0] << 4) | chunk.get(1).copied().unwrap_or(0));
            }
        }

        result
    }

    /// Decode hex-prefix encoded bytes back to nibbles.
    pub fn decode_hex_prefix(encoded: &[u8]) -> (Self, bool) {
        if encoded.is_empty() {
            return (Nibbles(vec![]), false);
        }

        let prefix = encoded[0] >> 4;
        let is_leaf = prefix >= 2;
        let odd = prefix % 2 == 1;

        let mut nibbles = Vec::new();

        if odd {
            nibbles.push(encoded[0] & 0x0F);
        }

        for &byte in &encoded[1..] {
            nibbles.push(byte >> 4);
            nibbles.push(byte & 0x0F);
        }

        (Nibbles(nibbles), is_leaf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nibbles_from_address() {
        let addr = [
            0xAB, 0xCD, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
        ];
        let nibbles = Nibbles::from_address(&addr);
        assert_eq!(nibbles.len(), 40);
        assert_eq!(nibbles.at(0), 0x0A);
        assert_eq!(nibbles.at(1), 0x0B);
        assert_eq!(nibbles.at(2), 0x0C);
        assert_eq!(nibbles.at(3), 0x0D);
        assert_eq!(nibbles.at(38), 0x0F);
        assert_eq!(nibbles.at(39), 0x0F);
    }

    #[test]
    fn test_hex_prefix_encoding() {
        // Even length leaf
        let nibbles = Nibbles(vec![1, 2, 3, 4]);
        let encoded = nibbles.encode_hex_prefix(true);
        assert_eq!(encoded[0] >> 4, 2); // Leaf flag, even

        // Odd length leaf
        let nibbles = Nibbles(vec![1, 2, 3]);
        let encoded = nibbles.encode_hex_prefix(true);
        assert_eq!(encoded[0] >> 4, 3); // Leaf flag, odd

        // Even length extension
        let nibbles = Nibbles(vec![1, 2, 3, 4]);
        let encoded = nibbles.encode_hex_prefix(false);
        assert_eq!(encoded[0] >> 4, 0); // Extension flag, even
    }

    #[test]
    fn test_hex_prefix_roundtrip() {
        let original = Nibbles(vec![1, 2, 3, 4, 5]);
        let encoded = original.encode_hex_prefix(true);
        let (decoded, is_leaf) = Nibbles::decode_hex_prefix(&encoded);
        assert!(is_leaf);
        assert_eq!(decoded.0, original.0);
    }
}
