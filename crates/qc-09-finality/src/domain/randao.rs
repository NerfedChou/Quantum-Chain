//! # RANDAO Randomness
//!
//! Unbiasable randomness for committee shuffling using BLS signatures.
//!
//! ## Threat
//!
//! Validators manipulating committee assignments to gain supermajority in a shard.
//!
//! ## Solution: RANDAO Mix
//!
//! 1. Each block includes a BLS signature of the epoch number (unforgeable)
//! 2. Mix = XOR(All_Signatures_In_Epoch)
//! 3. Use Mix as seed to shuffle validator set for next epoch
//!
//! Reference: SPEC-09-FINALITY.md, Ethereum Beacon Chain

use shared_types::Hash;

/// RANDAO accumulator for unbiasable randomness.
#[derive(Clone, Debug)]
pub struct RandaoAccumulator {
    /// Current epoch
    current_epoch: u64,
    /// Accumulated RANDAO mix
    mix: Hash,
    /// Number of contributions this epoch
    contributions: u64,
}

impl RandaoAccumulator {
    /// Create new accumulator for an epoch.
    pub fn new(epoch: u64, initial_mix: Hash) -> Self {
        Self {
            current_epoch: epoch,
            mix: initial_mix,
            contributions: 0,
        }
    }

    /// Mix in a new RANDAO reveal (BLS signature over epoch).
    pub fn mix_in(&mut self, reveal: &[u8; 32]) {
        self.mix = xor_hashes(&self.mix, reveal);
        self.contributions += 1;
    }

    /// Get the current mix.
    pub fn get_mix(&self) -> Hash {
        self.mix
    }

    /// Get number of contributions.
    pub fn contributions(&self) -> u64 {
        self.contributions
    }

    /// Get current epoch.
    pub fn epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Advance to next epoch with seed from current mix.
    pub fn advance_epoch(&mut self) -> Hash {
        let next_seed = self.mix;
        self.current_epoch += 1;
        self.mix = hash_to_seed(&next_seed, self.current_epoch);
        self.contributions = 0;
        next_seed
    }
}

impl Default for RandaoAccumulator {
    fn default() -> Self {
        Self::new(0, [0u8; 32])
    }
}

/// XOR two hashes.
fn xor_hashes(a: &Hash, b: &Hash) -> Hash {
    let mut result = [0u8; 32];
    for i in 0..32 {
        result[i] = a[i] ^ b[i];
    }
    result
}

/// Generate seed for an epoch from previous mix.
fn hash_to_seed(mix: &Hash, epoch: u64) -> Hash {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    hasher.update(mix);
    hasher.update(epoch.to_le_bytes());
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Shuffle a list of items using RANDAO seed (Fisher-Yates).
pub fn shuffle_with_seed<T: Clone>(items: &[T], seed: &Hash) -> Vec<T> {
    let mut result = items.to_vec();
    let len = result.len();

    if len <= 1 {
        return result;
    }

    // Use seed to generate random indices
    let mut rng_state = *seed;

    for i in (1..len).rev() {
        // Generate pseudo-random index
        rng_state = hash_to_seed(&rng_state, i as u64);
        let j = u64::from_le_bytes(rng_state[0..8].try_into().unwrap()) as usize % (i + 1);
        result.swap(i, j);
    }

    result
}

/// Compute committee assignments for an epoch.
pub fn compute_committees<T: Clone>(
    validators: &[T],
    seed: &Hash,
    committee_count: usize,
) -> Vec<Vec<T>> {
    if committee_count == 0 {
        return Vec::new();
    }

    let shuffled = shuffle_with_seed(validators, seed);
    let validators_per_committee = (shuffled.len() + committee_count - 1) / committee_count;

    shuffled
        .chunks(validators_per_committee)
        .map(|chunk| chunk.to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_in() {
        let mut acc = RandaoAccumulator::new(0, [0u8; 32]);

        acc.mix_in(&[1u8; 32]);
        assert_eq!(acc.contributions(), 1);
        assert_eq!(acc.get_mix(), [1u8; 32]);

        acc.mix_in(&[1u8; 32]);
        assert_eq!(acc.contributions(), 2);
        assert_eq!(acc.get_mix(), [0u8; 32]); // XOR cancels out
    }

    #[test]
    fn test_advance_epoch() {
        let mut acc = RandaoAccumulator::new(5, [0xAB; 32]);

        acc.mix_in(&[0x11; 32]);
        let seed = acc.advance_epoch();

        assert_eq!(acc.epoch(), 6);
        assert_eq!(acc.contributions(), 0);
        assert_ne!(seed, [0u8; 32]);
    }

    #[test]
    fn test_shuffle_deterministic() {
        let items = vec![1, 2, 3, 4, 5];
        let seed = [0xAB; 32];

        let result1 = shuffle_with_seed(&items, &seed);
        let result2 = shuffle_with_seed(&items, &seed);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_shuffle_different_seeds() {
        let items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        let result1 = shuffle_with_seed(&items, &[0xAA; 32]);
        let result2 = shuffle_with_seed(&items, &[0xBB; 32]);

        assert_ne!(result1, result2);
    }

    #[test]
    fn test_compute_committees() {
        let validators: Vec<u32> = (0..100).collect();
        let seed = [0xCD; 32];

        let committees = compute_committees(&validators, &seed, 4);

        assert_eq!(committees.len(), 4);

        // All validators should be assigned
        let total: usize = committees.iter().map(|c| c.len()).sum();
        assert_eq!(total, 100);
    }
}
