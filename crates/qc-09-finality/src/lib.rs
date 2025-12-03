//! # qc-09-finality
//!
//! Finality Gadget implementing Casper FFG for economic finality guarantees.
//!
//! ## Overview
//!
//! This subsystem provides:
//! - **Casper FFG**: Two-phase finality (justified → finalized)
//! - **2/3 Threshold**: Supermajority stake required for justification
//! - **Circuit Breaker**: Livelock prevention with manual intervention
//! - **Zero-Trust**: Independent signature re-verification
//!
//! ## Architecture
//!
//! Reference: SPEC-09-FINALITY.md, Architecture.md v2.3
//!
//! ```text
//! Consensus (8) ──AttestationBatch──→ Finality (9)
//!                                         │
//!                                         ├── MarkFinalizedRequest ──→ Block Storage (2)
//!                                         │
//!                                         └── FinalityProof ──→ Cross-Chain (15)
//! ```
//!
//! ## Security Model
//!
//! Reference: IPC-MATRIX.md Subsystem 9
//!
//! | Message | Authorized Sender |
//! |---------|-------------------|
//! | AttestationBatch | Consensus (8) |
//! | FinalityCheckRequest | Consensus (8) |
//! | FinalityProofRequest | Cross-Chain (15) |
//!
//! ## Circuit Breaker
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
//! ## Example
//!
//! ```rust,ignore
//! use qc_09_finality::{FinalityService, FinalityConfig};
//! use qc_09_finality::ports::inbound::FinalityApi;
//!
//! let service = FinalityService::new(
//!     FinalityConfig::default(),
//!     block_storage,
//!     verifier,
//!     validator_provider,
//! );
//!
//! // Process attestations
//! let result = service.process_attestations(attestations).await?;
//!
//! // Check if block is finalized
//! let is_final = service.is_finalized(block_hash).await;
//! ```

pub mod domain;
pub mod error;
pub mod events;
pub mod ipc;
pub mod ports;
pub mod service;

pub use domain::proof::FinalityProof;
pub use domain::{
    AggregatedAttestations, Attestation, BlsSignature, Checkpoint, CheckpointId, CheckpointState,
    CircuitBreaker, FinalityEvent, FinalityState, ValidatorId, ValidatorSet,
};
pub use error::{FinalityError, FinalityResult};
pub use events::{AttestationBatch, FinalityAchievedEvent, MarkFinalizedPayload};
pub use ipc::FinalityIpcHandler;
pub use ports::inbound::{AttestationResult, FinalityApi};
pub use ports::outbound::{
    AttestationVerifier, BlockStorageGateway, MarkFinalizedRequest, ValidatorSetProvider,
};
pub use service::{FinalityConfig, FinalityService};
