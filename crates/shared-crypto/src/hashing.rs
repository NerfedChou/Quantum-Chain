//! # BLAKE3 Hashing
//!
//! Ultra-fast cryptographic hashing with SIMD acceleration.
//!
//! ## Performance
//!
//! - 5-10x faster than SHA-256
//! - Exploits AVX-512/NEON via internal Merkle tree

use blake3::Hasher;

/// BLAKE3 hash output (256-bit).
pub type Hash = [u8; 32];

/// Stateful BLAKE3 hasher.
pub struct Blake3Hasher {
    inner: Hasher,
}

impl Blake3Hasher {
    /// Create new hasher.
    pub fn new() -> Self {
        Self {
            inner: Hasher::new(),
        }
    }

    /// Create keyed hasher (for MAC).
    pub fn new_keyed(key: &[u8; 32]) -> Self {
        Self {
            inner: Hasher::new_keyed(key),
        }
    }

    /// Update with data.
    pub fn update(&mut self, data: &[u8]) -> &mut Self {
        self.inner.update(data);
        self
    }

    /// Finalize and return hash.
    pub fn finalize(&self) -> Hash {
        let hash = self.inner.finalize();
        *hash.as_bytes()
    }

    /// Reset hasher for reuse.
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl Default for Blake3Hasher {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash data with BLAKE3 (one-shot).
pub fn blake3_hash(data: &[u8]) -> Hash {
    *blake3::hash(data).as_bytes()
}

/// Hash multiple inputs.
pub fn blake3_hash_many(inputs: &[&[u8]]) -> Hash {
    let mut hasher = Blake3Hasher::new();
    for input in inputs {
        hasher.update(input);
    }
    hasher.finalize()
}

/// Keyed hash (MAC).
pub fn blake3_keyed_hash(key: &[u8; 32], data: &[u8]) -> Hash {
    *blake3::keyed_hash(key, data).as_bytes()
}

/// Derive key from context and input key material.
pub fn blake3_derive_key(context: &str, key_material: &[u8]) -> [u8; 32] {
    let mut output = [0u8; 32];
    let mut hasher = blake3::Hasher::new_derive_key(context);
    hasher.update(key_material);
    let hash = hasher.finalize();
    output.copy_from_slice(hash.as_bytes());
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hash() {
        let hash = blake3_hash(b"Hello, World!");
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_deterministic() {
        let h1 = blake3_hash(b"test");
        let h2 = blake3_hash(b"test");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_inputs() {
        let h1 = blake3_hash(b"input1");
        let h2 = blake3_hash(b"input2");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_streaming() {
        let hash_oneshot = blake3_hash(b"hello world");

        let mut hasher = Blake3Hasher::new();
        hasher.update(b"hello ");
        hasher.update(b"world");
        let hash_streaming = hasher.finalize();

        assert_eq!(hash_oneshot, hash_streaming);
    }

    #[test]
    fn test_keyed_hash() {
        let key = [0xABu8; 32];
        let h1 = blake3_keyed_hash(&key, b"data");
        let h2 = blake3_keyed_hash(&key, b"data");
        let h3 = blake3_keyed_hash(&[0xCDu8; 32], b"data");

        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_derive_key() {
        let key = blake3_derive_key("quantum-chain encryption", b"master secret");
        assert_eq!(key.len(), 32);
    }
}
