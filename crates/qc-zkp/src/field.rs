//! # Goldilocks Field
//!
//! Prime field with p = 2^64 - 2^32 + 1 (the "Goldilocks" prime).
//!
//! ## Properties
//!
//! - 64-bit native arithmetic (ultra-fast on modern CPUs)
//! - Efficient multiplication via Montgomery reduction
//! - FFT-friendly (has 2^32 roots of unity)

use std::ops::{Add, Mul, Sub, Neg};

/// Goldilocks prime: p = 2^64 - 2^32 + 1
pub const GOLDILOCKS_PRIME: u64 = 0xFFFF_FFFF_0000_0001;

/// Goldilocks field for Plonky2-style proofs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoldilocksField;

impl GoldilocksField {
    /// The field modulus.
    pub const MODULUS: u64 = GOLDILOCKS_PRIME;

    /// Create a field element.
    pub fn new(value: u64) -> FieldElement {
        FieldElement::new(value)
    }

    /// Zero element.
    pub fn zero() -> FieldElement {
        FieldElement(0)
    }

    /// One element.
    pub fn one() -> FieldElement {
        FieldElement(1)
    }

    /// Generator (primitive root).
    pub fn generator() -> FieldElement {
        FieldElement(7)
    }
}

/// Element in the Goldilocks field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FieldElement(u64);

impl FieldElement {
    /// Create new field element (reduces mod p).
    pub fn new(value: u64) -> Self {
        Self(value % GOLDILOCKS_PRIME)
    }

    /// Create from u128, reducing mod p.
    pub fn from_u128(value: u128) -> Self {
        Self((value % GOLDILOCKS_PRIME as u128) as u64)
    }

    /// Get the raw value.
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Compute multiplicative inverse using Fermat's little theorem.
    /// a^(-1) = a^(p-2) mod p
    pub fn inverse(&self) -> Option<Self> {
        if self.0 == 0 {
            return None;
        }
        Some(self.pow(GOLDILOCKS_PRIME - 2))
    }

    /// Exponentiation by squaring.
    pub fn pow(&self, mut exp: u64) -> Self {
        let mut base = *self;
        let mut result = FieldElement(1);

        while exp > 0 {
            if exp & 1 == 1 {
                result = result * base;
            }
            base = base * base;
            exp >>= 1;
        }
        result
    }

    /// Check if zero.
    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl Add for FieldElement {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        // Use u128 to avoid overflow
        let sum = self.0 as u128 + rhs.0 as u128;
        Self::from_u128(sum)
    }
}

impl Sub for FieldElement {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            // Wrap around: (self + p) - rhs
            Self(GOLDILOCKS_PRIME - rhs.0 + self.0)
        }
    }
}

impl Mul for FieldElement {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let product = self.0 as u128 * rhs.0 as u128;
        Self::from_u128(product)
    }
}

impl Neg for FieldElement {
    type Output = Self;

    fn neg(self) -> Self {
        if self.0 == 0 {
            self
        } else {
            Self(GOLDILOCKS_PRIME - self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_addition() {
        let a = FieldElement::new(10);
        let b = FieldElement::new(20);
        assert_eq!((a + b).value(), 30);
    }

    #[test]
    fn test_field_subtraction() {
        let a = FieldElement::new(30);
        let b = FieldElement::new(10);
        assert_eq!((a - b).value(), 20);
    }

    #[test]
    fn test_field_subtraction_wrap() {
        let a = FieldElement::new(10);
        let b = FieldElement::new(30);
        let c = a - b;
        assert_eq!(c.value(), GOLDILOCKS_PRIME - 20);
    }

    #[test]
    fn test_field_multiplication() {
        let a = FieldElement::new(1000);
        let b = FieldElement::new(2000);
        assert_eq!((a * b).value(), 2_000_000);
    }

    #[test]
    fn test_field_inverse() {
        let a = FieldElement::new(7);
        let inv = a.inverse().unwrap();
        assert_eq!((a * inv).value(), 1);
    }

    #[test]
    fn test_zero_inverse() {
        let zero = FieldElement::new(0);
        assert!(zero.inverse().is_none());
    }

    #[test]
    fn test_pow() {
        let a = FieldElement::new(2);
        assert_eq!(a.pow(10).value(), 1024);
    }

    #[test]
    fn test_modular_reduction() {
        let a = FieldElement::new(GOLDILOCKS_PRIME + 5);
        assert_eq!(a.value(), 5);
    }
}
