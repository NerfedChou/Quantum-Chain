//! # Signature Verification Subsystem (QC-10)
//!
//! Provides cryptographic signature verification for Quantum-Chain.
//!
//! ## Quick Start
//!
//! ```rust
//! use qc_10_signature_verification::{
//!     EcdsaSignature, keccak256, EcdsaVerifier,
//! };

#![warn(missing_docs)]
#![allow(missing_docs)]
// TODO: Add documentation for all public items

// Pedantic lints that are too strict for this crate
#![allow(clippy::manual_let_else)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::if_not_else)]
// Allow in tests
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]
#![cfg_attr(test, allow(clippy::uninlined_format_args))]
#![cfg_attr(test, allow(clippy::useless_asref))]
#![cfg_attr(test, allow(clippy::assigning_clones))]
//!
//! // Hash a message
//! let message = b"Hello, Quantum-Chain!";
//! let message_hash = keccak256(message);
//!
//! // Verify a signature using the domain layer directly
//! let signature = EcdsaSignature {
//!     r: [0u8; 32],
//!     s: [0u8; 32],
//!     v: 27,
//! };
//!
//! let verifier = EcdsaVerifier;
//! let result = verifier.verify_ecdsa(&message_hash, &signature);
//! println!("Valid: {}", result.valid);
//! ```
//!
//! ## Architecture
//!
//! This subsystem follows hexagonal architecture:
//! - **Domain Layer** (`domain/`): Pure cryptographic logic, no I/O
//! - **Ports Layer** (`ports/`): Trait definitions for inbound/outbound interfaces
//! - **Service Layer** (`service.rs`): Wires domain logic to ports
//! - **Adapters Layer** (`adapters/`): Infrastructure implementations (IPC, etc.)
//!
//! ## Specification Reference
//!
//! See `SPECS/SPEC-10-SIGNATURE-VERIFICATION.md` for complete technical specification.
//!
//! ## Security Considerations
//!
//! ### Malleability Prevention (EIP-2)
//!
//! All ECDSA signatures are checked for malleability. Signatures with S values
//! in the upper half of the curve order are rejected. This prevents transaction
//! malleability attacks where an attacker modifies a valid signature to create
//! a different but still valid signature.
//!
//! ### Zero-Trust Policy
//!
//! **CRITICAL:** Subsystems 8 (Consensus) and 9 (Finality) MUST NOT trust the
//! `signature_valid` flag blindly. They MUST re-verify signatures independently
//! before making consensus or finality decisions. See IPC-MATRIX.md for details.
//!
//! ### Authorized Consumers
//!
//! Only the following subsystems may request signature verification:
//! - Subsystem 1 (Peer Discovery) - Node identity verification only
//! - Subsystem 5 (Block Propagation) - Block signature verification
//! - Subsystem 6 (Mempool) - Transaction signature verification
//! - Subsystem 8 (Consensus) - All verification types + batch
//! - Subsystem 9 (Finality) - Attestation signature verification
//!
//! All other subsystems are explicitly forbidden (see IPC-MATRIX.md).
//!
//! ### Rate Limiting
//!
//! Per-subsystem rate limits are enforced to prevent denial-of-service attacks:
//! - Subsystem 1: 100 req/sec (network edge protection)
//! - Subsystems 5, 6: 1000 req/sec (internal traffic)
//! - Subsystems 8, 9: No limit (consensus-critical path)

pub mod adapters;
pub mod domain;
pub mod ports;
pub mod service;

// Re-export public API
pub use domain::bls::{
    aggregate_bls_public_keys, aggregate_bls_signatures, verify_bls, verify_bls_aggregate,
};
pub use domain::ecdsa::{address_from_pubkey, keccak256, EcdsaVerifier};
pub use domain::entities::{
    Address, BatchVerificationRequest, BatchVerificationResult, BlsPublicKey, BlsSignature,
    EcdsaPublicKey, EcdsaSignature, VerificationRequest, VerificationResult, VerifiedTransaction,
};
pub use domain::errors::SignatureError;
pub use ports::inbound::SignatureVerificationApi;
pub use ports::outbound::MempoolGateway;
pub use service::SignatureVerificationService;

// Re-export IPC handler and security constants
pub use adapters::ipc::{authorized, forbidden, IpcError, IpcHandler, RateLimits, SUBSYSTEM_ID};

// Re-export bus adapter for V2.3 choreography
pub use adapters::bus::{EventBusAdapter, SignatureVerificationBusAdapter};
