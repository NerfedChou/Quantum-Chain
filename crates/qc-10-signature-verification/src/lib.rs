//! # Signature Verification Subsystem (QC-10)
//!
//! Provides cryptographic signature verification for Quantum-Chain.
//!
//! ## Architecture
//!
//! This subsystem follows hexagonal architecture:
//! - **Domain Layer** (`domain/`): Pure cryptographic logic, no I/O
//! - **Ports Layer** (`ports/`): Trait definitions for inbound/outbound interfaces
//! - **Service Layer** (`service.rs`): Wires domain logic to ports
//!
//! ## Specification Reference
//!
//! See `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md` for complete technical specification.
//!
//! ## Security Notes
//!
//! - **Malleability Prevention (EIP-2)**: Signatures with high S values are rejected
//! - **Zero-Trust**: Subsystems 8 and 9 should re-verify signatures independently
//! - **Authorized Consumers**: Only subsystems 1, 5, 6, 8, 9 may request verification

pub mod domain;
pub mod ports;

// Re-export public API
pub use domain::bls::{aggregate_bls_signatures, verify_bls, verify_bls_aggregate};
pub use domain::ecdsa::{address_from_pubkey, keccak256, EcdsaVerifier};
pub use domain::entities::{
    Address, BatchVerificationRequest, BatchVerificationResult, BlsPublicKey, BlsSignature,
    EcdsaPublicKey, EcdsaSignature, VerificationRequest, VerificationResult, VerifiedTransaction,
};
pub use domain::errors::SignatureError;
pub use ports::inbound::SignatureVerificationApi;
pub use ports::outbound::MempoolGateway;
