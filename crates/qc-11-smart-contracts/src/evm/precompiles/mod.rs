//! # Precompiled Contracts
//!
//! Implementation of Ethereum precompiled contracts (0x01-0x09).

pub mod ecrecover;
pub mod identity;
pub mod modexp;
pub mod sha256;

use crate::domain::value_objects::{Address, Bytes};
use crate::errors::PrecompileError;

/// Precompile execution result.
pub struct PrecompileOutput {
    /// Gas used by the precompile.
    pub gas_used: u64,
    /// Output data.
    pub output: Bytes,
}

/// Trait for precompiled contracts.
pub trait Precompile: Send + Sync {
    /// Execute the precompile with given input.
    ///
    /// # Arguments
    ///
    /// * `input` - Input data
    /// * `gas_limit` - Maximum gas available
    ///
    /// # Returns
    ///
    /// * `PrecompileOutput` - Gas used and output data
    fn execute(&self, input: &[u8], gas_limit: u64) -> Result<PrecompileOutput, PrecompileError>;

    /// Get the address of this precompile.
    fn address(&self) -> Address;
}

/// Check if an address is a precompile and execute it.
#[must_use]
pub fn execute_precompile(
    address: Address,
    input: &[u8],
    gas_limit: u64,
) -> Option<Result<PrecompileOutput, PrecompileError>> {
    // Check if this is a precompile address (0x01 - 0x09)
    if !address.is_precompile() {
        return None;
    }

    let precompile_num = address.as_bytes()[19];

    let result = match precompile_num {
        1 => ecrecover::Ecrecover.execute(input, gas_limit),
        2 => sha256::Sha256Precompile.execute(input, gas_limit),
        3 => {
            // RIPEMD160 - simplified
            Err(PrecompileError::NotImplemented(address))
        }
        4 => identity::Identity.execute(input, gas_limit),
        5 => modexp::ModExp.execute(input, gas_limit),
        6..=9 => {
            // BN128 operations and Blake2f - simplified
            Err(PrecompileError::NotImplemented(address))
        }
        _ => return None,
    };

    Some(result)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_precompile_identity() {
        let mut addr = [0u8; 20];
        addr[19] = 4; // Identity precompile
        let address = Address::new(addr);

        let input = b"hello world";
        let result = execute_precompile(address, input, 100_000);

        assert!(result.is_some());
        let output = result.unwrap().unwrap();
        assert_eq!(output.output.as_slice(), input);
    }

    #[test]
    fn test_execute_precompile_not_precompile() {
        let address = Address::new([1u8; 20]); // Not a precompile
        let result = execute_precompile(address, b"test", 100_000);
        assert!(result.is_none());
    }
}
