//! # Identity Precompile (0x04)
//!
//! Simply returns the input data as output.

use super::{Precompile, PrecompileOutput};
use crate::domain::value_objects::{Address, Bytes};
use crate::errors::PrecompileError;

/// Gas cost per word.
const IDENTITY_WORD_COST: u64 = 3;
/// Base gas cost.
const IDENTITY_BASE_COST: u64 = 15;

/// Identity precompile - returns input as output.
pub struct Identity;

impl Precompile for Identity {
    fn execute(&self, input: &[u8], gas_limit: u64) -> Result<PrecompileOutput, PrecompileError> {
        // Calculate gas
        let word_size = (input.len() + 31) / 32;
        let gas_cost = IDENTITY_BASE_COST + IDENTITY_WORD_COST * word_size as u64;

        if gas_cost > gas_limit {
            return Err(PrecompileError::OutOfGas);
        }

        Ok(PrecompileOutput {
            gas_used: gas_cost,
            output: Bytes::from_slice(input),
        })
    }

    fn address(&self) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = 4;
        Address::new(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let precompile = Identity;
        let input = b"hello world";
        let result = precompile.execute(input, 100_000).unwrap();

        assert_eq!(result.output.as_slice(), input);
        assert!(result.gas_used > 0);
    }

    #[test]
    fn test_identity_empty() {
        let precompile = Identity;
        let result = precompile.execute(&[], 100_000).unwrap();
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_identity_out_of_gas() {
        let precompile = Identity;
        let input = [0u8; 100];
        let result = precompile.execute(&input, 1);
        assert!(matches!(result, Err(PrecompileError::OutOfGas)));
    }
}
