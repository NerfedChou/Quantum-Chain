//! # QC-ZKP: Zero-Knowledge Proofs
//!
//! Plonky2-style ZK proofs with Goldilocks field.
//!
//! ## Components
//!
//! - `field` - Goldilocks field arithmetic (p = 2^64 - 2^32 + 1)
//! - `polynomial` - Polynomial operations
//! - `commitment` - Merkle tree commitments
//! - `prover` - Proof generation
//! - `verifier` - Proof verification

#![warn(missing_docs)]

pub mod commitment;
pub mod errors;
pub mod field;
pub mod polynomial;
pub mod proof;

pub use commitment::MerkleCommitment;
pub use errors::ZkpError;
pub use field::{FieldElement, GoldilocksField};
pub use polynomial::Polynomial;
pub use proof::{Proof, Prover, Verifier};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert!(!super::VERSION.is_empty());
    }
}
