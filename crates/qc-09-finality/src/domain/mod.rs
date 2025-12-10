//! Domain module for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 2
//!
//! Additional modules:
//! - inclusion: Attestation inclusion delay tracking
//! - batch_verifier: O(1) batch BLS verification

pub mod attestation;
pub mod batch_verifier;
pub mod checkpoint;
pub mod circuit_breaker;
pub mod inclusion;
pub mod proof;
pub mod validator;

pub use attestation::{AggregatedAttestations, Attestation, BlsSignature};
pub use batch_verifier::{BatchVerificationResult, BatchVerifier, BATCH_THRESHOLD};
pub use checkpoint::{Checkpoint, CheckpointId, CheckpointState};
pub use circuit_breaker::{CircuitBreaker, FinalityEvent, FinalityState};
pub use inclusion::{InclusionDelayTracker, InclusionRecord, RewardCurve, MAX_INCLUSION_DELAY};
pub use proof::FinalityProof;
pub use validator::{Validator, ValidatorId, ValidatorSet};

