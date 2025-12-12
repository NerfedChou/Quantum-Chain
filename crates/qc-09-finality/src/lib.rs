//! # QC-09 Finality - Economic Finality Guarantees Subsystem
//!
//! **Subsystem ID:** 9  
//! **Specification:** SPEC-09-FINALITY.md v2.3  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Provides economic finality guarantees using Casper FFG (Friendly Finality
//! Gadget). Once a block is finalized, reverting it requires burning at least
//! 1/3 of the total stake—providing strong security for high-value transactions.
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | Finalization Requires Justification | `service.rs:384-407` - `check_finalization()` |
//! | INVARIANT-2 | Justification Threshold (2/3 stake) | `domain/checkpoint.rs:try_justify()` |
//! | INVARIANT-3 | No Conflicting Finality (slashing) | `service.rs:291-317` - `check_slashable_conditions()` |
//! | INVARIANT-4 | Circuit Breaker Determinism | `domain/circuit_breaker.rs:132-159` - `next_state()` |
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `shared-types::security` for HMAC/nonce
//! - **Envelope-Only Identity**: Identity derived solely from `sender_id`
//! - **Replay Prevention**: Nonce caching via `NonceCache`
//!
//! ### Zero-Trust Signature Re-Verification (CRITICAL)
//!
//! Per IPC-MATRIX.md, Finality MUST NOT trust pre-validation flags.
//! All attestation signatures are independently re-verified:
//!
//! **Enforcement:** `service.rs:252-257` - `always_reverify_signatures` config
//!
//! ### IPC Authorization Matrix
//!
//! | Message | Authorized Sender(s) | Enforcement |
//! |---------|---------------------|-------------|
//! | `AttestationBatch` | Consensus (8) ONLY | `ipc/handler.rs:87-91` |
//! | `FinalityCheckRequest` | Consensus (8) ONLY | `ipc/handler.rs:115-119` |
//! | `FinalityProofRequest` | Cross-Chain (15) ONLY | `ipc/handler.rs:140-144` |
//!
//! ### Slashable Offense Detection
//!
//! | Offense Type | Description | Enforcement |
//! |--------------|-------------|-------------|
//! | DoubleVote | Same target epoch, different target block | `domain/attestation.rs:conflicts_with()` |
//! | SurroundVote | One attestation surrounds another | `domain/attestation.rs:surrounds()` |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | 2 (Block Storage) | `BlockStorageGateway` | Send `MarkFinalizedRequest` |
//! | 4 (State Mgmt) | `ValidatorSetProvider` | Get validator stake at epoch |
//! | 10 (Sig Verify) | `AttestationVerifier` | Re-verify BLS signatures |
//!
//! ## Circuit Breaker (Livelock Prevention)
//!
//! Reference: Architecture.md Section 5.4.1
//!
//! ```text
//! [RUNNING] ──failure──→ [SYNC {1}] ──fail──→ [SYNC {2}] ──fail──→ [SYNC {3}] ──fail──→ [HALTED]
//!     ↑                      │                    │                    │                    │
//!     └──────────────────────┴────────────────────┴────────────────────┘                    │
//!                                      (sync success)                                       │
//!     ↑                                                                                     │
//!     └─────────────────────────── manual intervention ─────────────────────────────────────┘
//! ```
//!
//! **Why Circuit Breaker:** Prevents infinite retry loops when finality fails.
//! If >33% validators reject, consensus is mathematically impossible.
//!
//! ## Casper FFG Two-Phase Finality
//!
//! | Phase | Threshold | Implementation |
//! |-------|-----------|----------------|
//! | Justification | 2/3 stake | `checkpoint.rs:try_justify()` |
//! | Finalization | Two consecutive justified | `service.rs:check_finalization()` |
//!
//! ## Inactivity Leak
//!
//! When finality stalls for `inactivity_leak_epochs` (default: 4), inactive
//! validators lose stake at `inactivity_leak_rate_bps` (default: 100 = 1%/epoch).
//!
//! **Enforcement:** `service.rs:613-627`
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use qc_09_finality::{FinalityService, FinalityConfig};
//! use qc_09_finality::ports::inbound::FinalityApi;
//!
//! let service = FinalityService::new(
//!     FinalityConfig::default(), block_storage, verifier, validator_provider,
//! );
//!
//! // Process attestations (Zero-Trust re-verifies all signatures)
//! let result = service.process_attestations(attestations).await?;
//!
//! // Forward slashing events to enforcement
//! for event in result.slashing_events { /* ... */ }
//! ```

#![warn(missing_docs)]
#![allow(missing_docs)] // TODO: Add documentation for all public items

pub mod domain;
pub mod error;
pub mod events;
pub mod ipc;
pub mod metrics;
pub mod ports;
pub mod service;

pub use domain::proof::FinalityProof;
pub use domain::{
    AggregatedAttestations, Attestation, BlsSignature, Checkpoint, CheckpointId, CheckpointState,
    CircuitBreaker, FinalityEvent, FinalityState, ValidatorId, ValidatorSet,
};
pub use error::{FinalityError, FinalityResult};
pub use events::{
    AttestationBatch, FinalityAchievedEvent, InactivityLeakTriggeredEvent, MarkFinalizedPayload,
    SlashableOffenseDetectedEvent,
};
pub use ipc::FinalityIpcHandler;
pub use ports::inbound::{AttestationResult, FinalityApi};
pub use ports::outbound::{
    AttestationVerifier, BlockStorageGateway, MarkFinalizedRequest, ValidatorSetProvider,
};
pub use service::{FinalityConfig, FinalityService};
