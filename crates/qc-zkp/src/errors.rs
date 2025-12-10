//! ZKP error types.

use thiserror::Error;

/// Zero-knowledge proof errors.
#[derive(Debug, Error)]
pub enum ZkpError {
    /// Invalid field element
    #[error("Invalid field element: value exceeds modulus")]
    InvalidFieldElement,

    /// Polynomial degree too high
    #[error("Polynomial degree {0} exceeds maximum {1}")]
    PolynomialDegreeTooHigh(usize, usize),

    /// Proof verification failed
    #[error("Proof verification failed")]
    VerificationFailed,

    /// Invalid commitment
    #[error("Invalid Merkle commitment")]
    InvalidCommitment,

    /// Witness mismatch
    #[error("Witness does not satisfy constraints")]
    WitnessMismatch,
}
