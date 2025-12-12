//! # Polynomial Operations
//!
//! Polynomial arithmetic over the Goldilocks field.

use crate::field::FieldElement;

/// Polynomial represented as coefficients.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Polynomial {
    coeffs: Vec<FieldElement>,
}

impl Polynomial {
    /// Create polynomial from coefficients (lowest degree first).
    pub fn new(coeffs: Vec<FieldElement>) -> Self {
        let mut p = Self { coeffs };
        p.normalize();
        p
    }

    /// Create zero polynomial.
    pub fn zero() -> Self {
        Self { coeffs: vec![] }
    }

    /// Create constant polynomial.
    pub fn constant(c: FieldElement) -> Self {
        if c.is_zero() {
            Self::zero()
        } else {
            Self { coeffs: vec![c] }
        }
    }

    /// Get degree (-1 for zero polynomial).
    pub fn degree(&self) -> isize {
        if self.coeffs.is_empty() {
            -1
        } else {
            (self.coeffs.len() - 1) as isize
        }
    }

    /// Evaluate polynomial at point.
    pub fn evaluate(&self, x: FieldElement) -> FieldElement {
        if self.coeffs.is_empty() {
            return FieldElement::new(0);
        }

        // Horner's method
        let mut result = *self.coeffs.last().unwrap();
        for coeff in self.coeffs.iter().rev().skip(1) {
            result = result * x + *coeff;
        }
        result
    }

    /// Get coefficients.
    pub fn coefficients(&self) -> &[FieldElement] {
        &self.coeffs
    }

    /// Remove leading zeros.
    fn normalize(&mut self) {
        while self.coeffs.last().is_some_and(|c| c.is_zero()) {
            self.coeffs.pop();
        }
    }

    /// Add two polynomials.
    pub fn add(&self, other: &Self) -> Self {
        let max_len = self.coeffs.len().max(other.coeffs.len());
        let mut result = Vec::with_capacity(max_len);

        for i in 0..max_len {
            let a = self.coeffs.get(i).copied().unwrap_or(FieldElement::new(0));
            let b = other.coeffs.get(i).copied().unwrap_or(FieldElement::new(0));
            result.push(a + b);
        }

        Self::new(result)
    }

    /// Multiply two polynomials.
    pub fn mul(&self, other: &Self) -> Self {
        if self.coeffs.is_empty() || other.coeffs.is_empty() {
            return Self::zero();
        }

        let result_len = self.coeffs.len() + other.coeffs.len() - 1;
        let mut result = vec![FieldElement::new(0); result_len];

        for (i, a) in self.coeffs.iter().enumerate() {
            for (j, b) in other.coeffs.iter().enumerate() {
                result[i + j] = result[i + j] + (*a * *b);
            }
        }

        Self::new(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate() {
        // p(x) = 1 + 2x + 3x^2
        let p = Polynomial::new(vec![
            FieldElement::new(1),
            FieldElement::new(2),
            FieldElement::new(3),
        ]);

        // p(2) = 1 + 4 + 12 = 17
        assert_eq!(p.evaluate(FieldElement::new(2)).value(), 17);
    }

    #[test]
    fn test_degree() {
        let p = Polynomial::new(vec![
            FieldElement::new(1),
            FieldElement::new(2),
            FieldElement::new(3),
        ]);
        assert_eq!(p.degree(), 2);
    }

    #[test]
    fn test_add() {
        let p1 = Polynomial::new(vec![FieldElement::new(1), FieldElement::new(2)]);
        let p2 = Polynomial::new(vec![FieldElement::new(3), FieldElement::new(4)]);
        let sum = p1.add(&p2);
        assert_eq!(sum.coefficients()[0].value(), 4);
        assert_eq!(sum.coefficients()[1].value(), 6);
    }

    #[test]
    fn test_mul() {
        // (1 + x) * (1 + x) = 1 + 2x + x^2
        let p = Polynomial::new(vec![FieldElement::new(1), FieldElement::new(1)]);
        let product = p.mul(&p);
        assert_eq!(product.coefficients()[0].value(), 1);
        assert_eq!(product.coefficients()[1].value(), 2);
        assert_eq!(product.coefficients()[2].value(), 1);
    }
}
