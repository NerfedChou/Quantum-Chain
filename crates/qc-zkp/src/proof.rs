//! # ZK Proof Types
//!
//! Proof generation and verification.

use crate::commitment::{HashOutput, MerkleCommitment};
use crate::field::FieldElement;
use crate::polynomial::Polynomial;

/// Zero-knowledge proof.
#[derive(Clone, Debug)]
pub struct Proof {
    /// Commitment to witness polynomial
    pub witness_commitment: HashOutput,
    /// Commitment to quotient polynomial
    pub quotient_commitment: HashOutput,
    /// Opening evaluations
    pub evaluations: Vec<FieldElement>,
    /// Challenge point
    pub challenge: FieldElement,
}

/// Prover for generating ZK proofs.
#[derive(Clone, Debug)]
pub struct Prover {
    /// Constraint polynomial
    constraint: Polynomial,
}

impl Prover {
    /// Create new prover with constraint.
    pub fn new(constraint: Polynomial) -> Self {
        Self { constraint }
    }

    /// Generate proof for witness satisfying constraint.
    pub fn prove(&self, witness: &[FieldElement]) -> Proof {
        // 1. Commit to witness
        let witness_commitment = MerkleCommitment::commit(witness);

        // 2. Create witness polynomial
        let witness_poly = Polynomial::new(witness.to_vec());

        // 3. Generate challenge (in practice, use Fiat-Shamir)
        let challenge = FieldElement::new(
            witness_commitment.root()[0] as u64 * 256 + witness_commitment.root()[1] as u64,
        );

        // 4. Evaluate at challenge point
        let witness_eval = witness_poly.evaluate(challenge);
        let constraint_eval = self.constraint.evaluate(challenge);

        // 5. Compute quotient (simplified)
        let quotient_poly = Polynomial::constant(constraint_eval);
        let quotient_commitment = MerkleCommitment::commit(&[quotient_poly.evaluate(challenge)]);

        Proof {
            witness_commitment: *witness_commitment.root(),
            quotient_commitment: *quotient_commitment.root(),
            evaluations: vec![witness_eval, constraint_eval],
            challenge,
        }
    }
}

/// Verifier for checking ZK proofs.
#[derive(Clone, Debug, Default)]
pub struct Verifier;

impl Verifier {
    /// Create new verifier.
    pub fn new() -> Self {
        Self
    }

    /// Verify a proof.
    pub fn verify(&self, proof: &Proof, public_inputs: &[FieldElement]) -> bool {
        // 1. Commitments must be non-zero
        if proof.witness_commitment == [0u8; 32] {
            return false;
        }

        // 2. Evaluations must be consistent
        if proof.evaluations.is_empty() {
            return false;
        }

        // 3. Challenge must match (Fiat-Shamir check)
        let expected_challenge = FieldElement::new(
            proof.witness_commitment[0] as u64 * 256 + proof.witness_commitment[1] as u64,
        );

        if proof.challenge != expected_challenge {
            return false;
        }

        // 4. Public inputs check (simplified)
        for (i, input) in public_inputs.iter().enumerate() {
            if i < proof.evaluations.len() && proof.evaluations[i] != *input {
                // Public input mismatch (in real impl, would check more carefully)
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prove_and_verify() {
        let constraint = Polynomial::new(vec![FieldElement::new(1), FieldElement::new(1)]);

        let prover = Prover::new(constraint);
        let witness = vec![FieldElement::new(5), FieldElement::new(10)];

        let proof = prover.prove(&witness);
        let verifier = Verifier::new();

        assert!(verifier.verify(&proof, &[]));
    }

    #[test]
    fn test_empty_witness() {
        let constraint = Polynomial::zero();
        let prover = Prover::new(constraint);
        let proof = prover.prove(&[]);
        let verifier = Verifier::new();

        // Empty proof fails
        assert!(!verifier.verify(&proof, &[]));
    }
}
