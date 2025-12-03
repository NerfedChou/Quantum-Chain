//! Domain module for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 2

pub mod attestation;
pub mod checkpoint;
pub mod circuit_breaker;
pub mod proof;
pub mod validator;

pub use attestation::{AggregatedAttestations, Attestation, BlsSignature};
pub use checkpoint::{Checkpoint, CheckpointId, CheckpointState};
pub use circuit_breaker::{CircuitBreaker, FinalityEvent, FinalityState};
pub use proof::FinalityProof;
pub use validator::{ValidatorId, ValidatorSet};
