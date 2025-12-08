//! # Ecrecover Precompile (0x01)
//!
//! Recovers the signer address from an ECDSA signature.
//!
//! Input format (128 bytes):
//! - bytes 0-31: message hash
//! - bytes 32-63: v (recovery id, should be 27 or 28)
//! - bytes 64-95: r
//! - bytes 96-127: s

use super::{Precompile, PrecompileOutput};
use crate::domain::value_objects::{Address, Bytes};
use crate::errors::PrecompileError;

/// Fixed gas cost for ecrecover.
const ECRECOVER_GAS: u64 = 3000;

/// Ecrecover precompile.
pub struct Ecrecover;

impl Precompile for Ecrecover {
    fn execute(&self, input: &[u8], gas_limit: u64) -> Result<PrecompileOutput, PrecompileError> {
        if ECRECOVER_GAS > gas_limit {
            return Err(PrecompileError::OutOfGas);
        }

        // Pad input to 128 bytes
        let mut padded = [0u8; 128];
        let len = input.len().min(128);
        padded[..len].copy_from_slice(&input[..len]);

        // Extract components
        let hash = &padded[0..32];
        let v = &padded[32..64];
        let r = &padded[64..96];
        let s = &padded[96..128];

        // v should be 27 or 28
        let v_value = v[31];
        if v_value != 27 && v_value != 28 {
            // Invalid v, return empty
            return Ok(PrecompileOutput {
                gas_used: ECRECOVER_GAS,
                output: Bytes::new(),
            });
        }

        // Check r and s are valid (non-zero, less than secp256k1 order)
        let r_zero = r.iter().all(|&b| b == 0);
        let s_zero = s.iter().all(|&b| b == 0);
        if r_zero || s_zero {
            return Ok(PrecompileOutput {
                gas_used: ECRECOVER_GAS,
                output: Bytes::new(),
            });
        }

        // In a real implementation, we would use a crypto library like k256 or secp256k1
        // For now, return a placeholder (would integrate with Subsystem 10)
        // This is a simplified implementation that returns empty for now

        // Real implementation would:
        // 1. Use k256::ecdsa::recoverable::Signature
        // 2. Recover the public key
        // 3. Hash with keccak256 and take last 20 bytes

        Ok(PrecompileOutput {
            gas_used: ECRECOVER_GAS,
            output: Bytes::new(), // Simplified: would return recovered address
        })
    }

    fn address(&self) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = 1;
        Address::new(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecrecover_gas() {
        let precompile = Ecrecover;
        let input = [0u8; 128];
        let result = precompile.execute(&input, 100_000).unwrap();
        assert_eq!(result.gas_used, ECRECOVER_GAS);
    }

    #[test]
    fn test_ecrecover_out_of_gas() {
        let precompile = Ecrecover;
        let input = [0u8; 128];
        let result = precompile.execute(&input, 100);
        assert!(matches!(result, Err(PrecompileError::OutOfGas)));
    }

    #[test]
    fn test_ecrecover_invalid_v() {
        let precompile = Ecrecover;
        let mut input = [0u8; 128];
        input[63] = 30; // Invalid v
        let result = precompile.execute(&input, 100_000).unwrap();
        assert!(result.output.is_empty()); // Returns empty on invalid
    }
}
