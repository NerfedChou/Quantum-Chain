//! # ModExp Precompile (0x05)
//!
//! Arbitrary-precision modular exponentiation.
//!
//! Input format:
//! - bytes 0-31: length of base (Bsize)
//! - bytes 32-63: length of exponent (Esize)
//! - bytes 64-95: length of modulus (Msize)
//! - bytes 96-(96+Bsize): base
//! - bytes (96+Bsize)-(96+Bsize+Esize): exponent
//! - bytes (96+Bsize+Esize)-(96+Bsize+Esize+Msize): modulus

use super::{Precompile, PrecompileOutput};
use crate::domain::value_objects::{Address, Bytes, U256};
use crate::errors::PrecompileError;

/// Minimum gas cost.
const MODEXP_MIN_GAS: u64 = 200;

/// ModExp precompile.
pub struct ModExp;

impl Precompile for ModExp {
    fn execute(&self, input: &[u8], gas_limit: u64) -> Result<PrecompileOutput, PrecompileError> {
        // Parse lengths
        let base_len = parse_u256(&input, 0).as_usize();
        let exp_len = parse_u256(&input, 32).as_usize();
        let mod_len = parse_u256(&input, 64).as_usize();

        // Sanity check lengths
        if base_len > 1024 || exp_len > 1024 || mod_len > 1024 {
            return Err(PrecompileError::InvalidInput(
                "lengths too large".to_string(),
            ));
        }

        // Calculate gas (simplified formula per EIP-2565)
        let max_len = base_len.max(mod_len);
        let words = (max_len + 7) / 8;
        let multiplication_complexity = (words * words) as u64;

        // Get exponent for iteration count
        let exp_start = 96 + base_len;
        let iteration_count = calculate_iteration_count(input, exp_start, exp_len);

        let gas_cost = (multiplication_complexity * iteration_count.max(1) / 3).max(MODEXP_MIN_GAS);

        if gas_cost > gas_limit {
            return Err(PrecompileError::OutOfGas);
        }

        // Parse base, exp, modulus
        let base = parse_big_uint(input, 96, base_len);
        let exp = parse_big_uint(input, 96 + base_len, exp_len);
        let modulus = parse_big_uint(input, 96 + base_len + exp_len, mod_len);

        // Handle edge cases
        if mod_len == 0 {
            return Ok(PrecompileOutput {
                gas_used: gas_cost,
                output: Bytes::new(),
            });
        }

        // Perform modexp
        let result = if modulus.is_empty() || modulus.iter().all(|&b| b == 0) {
            vec![0u8; mod_len]
        } else {
            mod_exp(&base, &exp, &modulus, mod_len)
        };

        Ok(PrecompileOutput {
            gas_used: gas_cost,
            output: Bytes::from_vec(result),
        })
    }

    fn address(&self) -> Address {
        let mut addr = [0u8; 20];
        addr[19] = 5;
        Address::new(addr)
    }
}

/// Parse U256 from input at offset.
fn parse_u256(input: &[u8], offset: usize) -> U256 {
    let mut bytes = [0u8; 32];
    let end = (offset + 32).min(input.len());
    let start = offset.min(input.len());
    let len = end.saturating_sub(start);
    if len > 0 && start < input.len() {
        bytes[32 - len..].copy_from_slice(&input[start..end]);
    }
    U256::from_big_endian(&bytes)
}

/// Parse arbitrary-length big integer from input.
fn parse_big_uint(input: &[u8], offset: usize, len: usize) -> Vec<u8> {
    let mut result = vec![0u8; len];
    let end = (offset + len).min(input.len());
    let start = offset.min(input.len());
    let available = end.saturating_sub(start);
    if available > 0 && start < input.len() {
        result[len - available..].copy_from_slice(&input[start..end]);
    }
    result
}

/// Calculate iteration count based on exponent.
fn calculate_iteration_count(input: &[u8], exp_start: usize, exp_len: usize) -> u64 {
    if exp_len <= 32 {
        let exp = parse_u256(input, exp_start);
        if exp.is_zero() {
            0
        } else {
            (256 - exp.leading_zeros()) as u64
        }
    } else {
        // For large exponents, use first 32 bytes plus additional length
        let first_32 = parse_u256(input, exp_start);
        let extra_bits = ((exp_len - 32) * 8) as u64;
        if first_32.is_zero() {
            extra_bits
        } else {
            (256 - first_32.leading_zeros()) as u64 + extra_bits
        }
    }
}

/// Perform modular exponentiation.
/// This is a simplified implementation. Production should use a proper big integer library.
fn mod_exp(base: &[u8], exp: &[u8], modulus: &[u8], result_len: usize) -> Vec<u8> {
    // Convert to U256 for small numbers (simplified)
    if base.len() <= 32 && exp.len() <= 32 && modulus.len() <= 32 {
        let base_val = U256::from_big_endian(base);
        let exp_val = U256::from_big_endian(exp);
        let mod_val = U256::from_big_endian(modulus);

        if mod_val.is_zero() {
            return vec![0u8; result_len];
        }

        // Simple modexp for small values
        let mut result = U256::one();
        let mut base = base_val % mod_val;
        let mut exp = exp_val;

        while !exp.is_zero() {
            if exp.bit(0) {
                result = mulmod(result, base, mod_val);
            }
            exp >>= 1;
            base = mulmod(base, base, mod_val);
        }

        let mut output = vec![0u8; result_len];
        let mut bytes = [0u8; 32];
        result.to_big_endian(&mut bytes);
        let start = if result_len >= 32 { result_len - 32 } else { 0 };
        let copy_len = result_len.min(32);
        output[start..start + copy_len].copy_from_slice(&bytes[32 - copy_len..]);
        output
    } else {
        // For larger numbers, would need a proper big integer library
        vec![0u8; result_len]
    }
}

/// Modular multiplication avoiding overflow.
fn mulmod(a: U256, b: U256, m: U256) -> U256 {
    if m.is_zero() {
        return U256::zero();
    }
    // Use U512 to avoid overflow
    let a_big = primitive_types::U512::from(a);
    let b_big = primitive_types::U512::from(b);
    let m_big = primitive_types::U512::from(m);
    let result = (a_big * b_big) % m_big;

    // Convert back to U256
    let mut bytes = [0u8; 64];
    result.to_big_endian(&mut bytes);
    U256::from_big_endian(&bytes[32..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modexp_simple() {
        let precompile = ModExp;

        // base=2, exp=3, mod=5 -> 2^3 % 5 = 3
        let mut input = vec![0u8; 96 + 3];
        // base_len = 1
        input[31] = 1;
        // exp_len = 1
        input[63] = 1;
        // mod_len = 1
        input[95] = 1;
        // base = 2
        input[96] = 2;
        // exp = 3
        input[97] = 3;
        // mod = 5
        input[98] = 5;

        let result = precompile.execute(&input, 100_000).unwrap();
        assert_eq!(result.output.len(), 1);
        assert_eq!(result.output.as_slice()[0], 3);
    }

    #[test]
    fn test_modexp_zero_modulus() {
        let precompile = ModExp;

        let mut input = vec![0u8; 96 + 3];
        input[31] = 1; // base_len
        input[63] = 1; // exp_len
        input[95] = 0; // mod_len = 0

        let result = precompile.execute(&input, 100_000).unwrap();
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_modexp_out_of_gas() {
        let precompile = ModExp;
        let input = vec![0u8; 96];
        let result = precompile.execute(&input, 10);
        assert!(matches!(result, Err(PrecompileError::OutOfGas)));
    }
}
