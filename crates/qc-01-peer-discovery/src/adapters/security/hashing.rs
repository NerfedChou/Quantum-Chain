//! Secure Hasher Adapters

use crate::ports::SecureHasher;

// Import RandomSource correctly to avoid ambiguity if it were re-exported
use super::random::OsRandomSource;
use crate::ports::RandomSource;

/// Simple hasher for testing (NOT DoS-resistant).
///
/// Uses a basic multiplicative hash for deterministic test behavior.
#[derive(Debug, Clone)]
pub struct SimpleHasher {
    key: u64,
}

impl SimpleHasher {
    /// Create a simple hasher with given key.
    pub fn new(key: u64) -> Self {
        Self { key }
    }

    /// Create with default key (0).
    pub fn default_key() -> Self {
        Self::new(0)
    }
}

impl Default for SimpleHasher {
    fn default() -> Self {
        Self::default_key()
    }
}

impl SecureHasher for SimpleHasher {
    fn hash(&self, data: &[u8]) -> u64 {
        let mut hash = self.key;
        for &byte in data {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }

    fn hash_combined(&self, a: &[u8], b: &[u8]) -> u64 {
        let mut hash = self.key;
        for &byte in a {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        for &byte in b {
            hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        hash
    }
}

/// SipHash-based secure hasher (DoS-resistant).
///
/// Uses SipHash-2-4 which is designed to be:
/// - Fast for short inputs
/// - Resistant to hash-flooding DoS attacks
///
/// # Key Management
///
/// The key should be:
/// - Generated randomly on node startup
/// - NOT derived from predictable values
/// - Kept secret (not transmitted)
#[derive(Debug, Clone)]
pub struct SipHasher {
    key: [u8; 16],
}

impl SipHasher {
    /// Create a SipHasher with the given 128-bit key.
    pub fn new(key: [u8; 16]) -> Self {
        Self { key }
    }

    /// Create with random key (using OsRandomSource).
    pub fn with_random_key() -> Self {
        let rng = OsRandomSource::new();
        let mut key = [0u8; 16];
        rng.shuffle_slice(&mut key);
        // Fill with random-ish values
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = rng.random_usize(256) as u8 ^ (i as u8);
        }
        Self { key }
    }

    /// SipHash-2-4 implementation.
    fn siphash(&self, data: &[u8]) -> u64 {
        // Extract key halves
        let k0 = u64::from_le_bytes([
            self.key[0],
            self.key[1],
            self.key[2],
            self.key[3],
            self.key[4],
            self.key[5],
            self.key[6],
            self.key[7],
        ]);
        let k1 = u64::from_le_bytes([
            self.key[8],
            self.key[9],
            self.key[10],
            self.key[11],
            self.key[12],
            self.key[13],
            self.key[14],
            self.key[15],
        ]);

        // SipHash initialization
        let mut v0 = k0 ^ 0x736f6d6570736575;
        let mut v1 = k1 ^ 0x646f72616e646f6d;
        let mut v2 = k0 ^ 0x6c7967656e657261;
        let mut v3 = k1 ^ 0x7465646279746573;

        let len = data.len();
        let blocks = len / 8;

        // Process 8-byte blocks
        for i in 0..blocks {
            let offset = i * 8;
            let m = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            v3 ^= m;
            Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
            Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
            v0 ^= m;
        }

        // Process remaining bytes
        let mut last = (len as u64) << 56;
        let remaining = &data[blocks * 8..];
        for (i, &byte) in remaining.iter().enumerate() {
            last |= (byte as u64) << (i * 8);
        }

        v3 ^= last;
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        v0 ^= last;

        // Finalization
        v2 ^= 0xff;
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);
        Self::sipround(&mut v0, &mut v1, &mut v2, &mut v3);

        v0 ^ v1 ^ v2 ^ v3
    }

    #[inline]
    fn sipround(v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64) {
        *v0 = v0.wrapping_add(*v1);
        *v1 = v1.rotate_left(13);
        *v1 ^= *v0;
        *v0 = v0.rotate_left(32);
        *v2 = v2.wrapping_add(*v3);
        *v3 = v3.rotate_left(16);
        *v3 ^= *v2;
        *v0 = v0.wrapping_add(*v3);
        *v3 = v3.rotate_left(21);
        *v3 ^= *v0;
        *v2 = v2.wrapping_add(*v1);
        *v1 = v1.rotate_left(17);
        *v1 ^= *v2;
        *v2 = v2.rotate_left(32);
    }
}

impl SecureHasher for SipHasher {
    fn hash(&self, data: &[u8]) -> u64 {
        self.siphash(data)
    }

    fn hash_combined(&self, a: &[u8], b: &[u8]) -> u64 {
        // Concatenate and hash
        let mut combined = Vec::with_capacity(a.len() + b.len());
        combined.extend_from_slice(a);
        combined.extend_from_slice(b);
        self.siphash(&combined)
    }
}
