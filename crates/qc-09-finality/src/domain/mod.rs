//! Domain module for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 2
//!
//! ## Core Modules
//! - attestation: Validator attestations
//! - checkpoint: Finality checkpoints
//! - circuit_breaker: Livelock prevention
//! - proof: Finality proofs
//! - validator: Validator set management
//!
//! ## Advanced Features
//! - inclusion: Timely attestation rewards
//! - batch_verifier: O(1) batch BLS verification
//! - inactivity_leak: Quadratic stake drain for liveness
//! - slashing_db: Casper commandment enforcement
//! - reversion_shield: Protection against finality reversion
//! - randao: Unbiasable randomness for committees
//! - committee_cache: Pre-aggregated BLS keys

pub mod attestation;
pub mod batch_verifier;
pub mod checkpoint;
pub mod circuit_breaker;
pub mod committee_cache;
pub mod inactivity_leak;
pub mod inclusion;
pub mod proof;
pub mod randao;
pub mod reversion_shield;
pub mod slashing_db;
pub mod validator;

// Core exports
pub use attestation::{AggregatedAttestations, Attestation, BlsSignature};
pub use checkpoint::{Checkpoint, CheckpointId, CheckpointState};
pub use circuit_breaker::{CircuitBreaker, FinalityEvent, FinalityState};
pub use proof::FinalityProof;
pub use validator::{Validator, ValidatorId, ValidatorSet};

// Advanced feature exports
pub use batch_verifier::{BatchVerificationResult, BatchVerifier, BATCH_THRESHOLD};
pub use committee_cache::{CommitteeKeyCache, ParticipationAnalysis, COMMITTEE_SIZE};
pub use inactivity_leak::{InactivityLeakConfig, InactivityLeakTracker, InactivityScore};
pub use inclusion::{InclusionDelayTracker, InclusionRecord, RewardCurve, MAX_INCLUSION_DELAY};
pub use randao::{compute_committees, shuffle_with_seed, RandaoAccumulator};
pub use reversion_shield::ReversionShield;
pub use slashing_db::{SlashingDb, SlashingEvidence, VoteRecord};
