//! # Adapters Layer (Hexagonal Architecture)
//!
//! Implements outbound port traits for integration with other subsystems.
//!
//! Reference: SPEC-09-FINALITY.md Section 7, Architecture.md Section 2.3

mod block_storage;
mod signature_verifier;
mod validator_provider;

pub use block_storage::{EventBusBlockStorageAdapter, MockBlockStorageAdapter};
pub use signature_verifier::BLSAttestationVerifier;
pub use validator_provider::StateManagementValidatorProvider;
