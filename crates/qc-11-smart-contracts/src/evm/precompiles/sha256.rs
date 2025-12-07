//! # SHA256 Precompile (0x02)
//!
//! Computes SHA-256 hash of input.

use super::{Precompile, PrecompileOutput};
use crate::domain::value_objects::{Address, Bytes};
use crate::errors::PrecompileError;
use sha2::{Digest, Sha256};

/// Gas cost per word.
const SHA256_WORD_COST: u64 = 12;
/// Base gas cost.
const SHA256_BASE_COST: u64 = 60;

/// SHA256 precompile.
pub struct Sha256Precompile;

impl Precompile for Sha256Precompile {
    fn execute(&self, input: &[u8], gas_limit: u64) -> Result<PrecompileOutput, PrecompileError> {
        // Calculate gas
        let word_size = (input.len() + 31) / 32;
        let gas_cost = SHA256_BASE_COST + SHA256_WORD_COST * word_size as u64;

        if gas_cost > gas_limit {
            return Err(PrecompileError::OutOfGas);
        }

        let hash = Sha256::digest(input);
        let output = Bytes::from_slice(&hash);

        Ok(PrecompileOutput {
            gas_used: gas_cost,
            output,
        })
    }

    fn address(&self) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = 2;
        Address::new(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_empty() {
        let precompile = Sha256Precompile;
        let result = precompile.execute(&[], 100_000).unwrap();

        // SHA256 of empty string
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14,
            0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
            0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
            0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(result.output.as_slice(), &expected);
    }

    #[test]
    fn test_sha256_hello() {
        let precompile = Sha256Precompile;
        let result = precompile.execute(b"hello", 100_000).unwrap();
        assert_eq!(result.output.len(), 32);
    }

    #[test]
    fn test_sha256_out_of_gas() {
        let precompile = Sha256Precompile;
        let result = precompile.execute(&[0u8; 100], 1);
        assert!(matches!(result, Err(PrecompileError::OutOfGas)));
    }
}
