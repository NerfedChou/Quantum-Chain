//! # QC-08 Consensus - Block Validation & Agreement Subsystem
//!
//! **Subsystem ID:** 8  
//! **Specification:** SPEC-08-CONSENSUS.md v2.3  
//! **Architecture:** Architecture.md v2.3, IPC-MATRIX.md v2.3  
//! **Status:** Production-Ready
//!
//! ## Purpose
//!
//! Achieves agreement on valid blocks across all network nodes by validating
//! blocks cryptographically and publishing `BlockValidated` events to trigger
//! the V2.3 choreography pattern.
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | Valid Parent | `service.rs:149-163` - `validate_parent()` |
//! | INVARIANT-2 | Sufficient Attestations (2/3 PoS) | `service.rs:356-366` - threshold check |
//! | INVARIANT-3 | Valid Signatures (ZERO-TRUST) | `service.rs:306-354` - re-verify all |
//! | INVARIANT-4 | Sequential Height | `service.rs:169-193` - `validate_height()` |
//! | INVARIANT-5 | Timestamp Ordering | `service.rs:198-224` - `validate_timestamp()` |
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `shared-types::security::MessageVerifier`
//! - **Envelope-Only Identity**: Identity derived solely from `sender_id`
//! - **Replay Prevention**: Nonce caching via `NonceCache`
//!
//! ### Zero-Trust Signature Re-Verification (CRITICAL)
//!
//! Per IPC-MATRIX.md, Consensus MUST NOT trust pre-validation flags from
//! Subsystem 10. All signatures are independently re-verified:
//!
//! ```text
//! Even if Subsystem 10 says signature_valid=true, we re-verify because
//! if Subsystem 10 is compromised, attackers could inject fake attestations.
//! ```
//!
//! **Enforcement:** `service.rs:306-354` (PoS), `service.rs:420-488` (PBFT)
//!
//! ### IPC Authorization Matrix
//!
//! | Message | Authorized Sender(s) | Enforcement |
//! |---------|---------------------|-------------|
//! | `ValidateBlockRequest` | Block Propagation (5) ONLY | `ipc/handler.rs:130-134` |
//! | `AttestationReceived` | Signature Verify (10) ONLY | `ipc/handler.rs:157-161` |
//!
//! ### Additional Security Defenses
//!
//! | Defense | Description | Enforcement |
//! |---------|-------------|-------------|
//! | Duplicate Vote Detection | Reject multiple votes from same validator | `service.rs:296-301` |
//! | Future Timestamp Rejection | Blocks too far in future rejected | `service.rs:201-207` |
//! | Extra Data Size Limit | Max 32 bytes to prevent DoS | `service.rs:133-141` |
//! | Gas Limit Enforcement | Block gas checked against limit | `service.rs:116-131` |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | Event Bus | `EventBus` | Publish `BlockValidated` (choreography) |
//! | 6 (Mempool) | `MempoolGateway` | Get transactions for block building |
//! | 10 (Sig Verify) | `SignatureVerifier` | Re-verify signatures (Zero-Trust) |
//! | 4 (State Mgmt) | `ValidatorSetProvider` | Validator set at epoch boundary |
//!
//! ## V2.3 Choreography Pattern (NOT Orchestrator)
//!
//! ```text
//! Consensus (8) ──BlockValidated──→ [Event Bus]
//!                                        │
//!                  ┌─────────────────────┼─────────────────────┐
//!                  ↓                     ↓                     ↓
//!         [Tx Indexing (3)]    [State Mgmt (4)]    [Block Storage (2)]
//! ```
//!
//! Consensus performs validation ONLY - it does NOT orchestrate block storage.
//!
//! ## Consensus Algorithms
//!
//! | Algorithm | Threshold | Message Type |
//! |-----------|-----------|--------------|
//! | PoS | 2/3 attestations | Aggregate BLS/ECDSA |
//! | PBFT | 2f+1 prepare + commit | Per-message ECDSA |
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use qc_08_consensus::{ConsensusService, ConsensusConfig};
//!
//! let service = ConsensusService::new(
//!     event_bus, mempool, sig_verifier, validator_provider,
//!     ConsensusConfig::default(),
//! );
//!
//! // Validate a block (ZERO-TRUST re-verifies all signatures)
//! let validated = service.validate_block(block, None).await?;
//! ```

pub mod adapters;
pub mod domain;
pub mod events;
pub mod ipc;
pub mod metrics;
pub mod ports;
pub mod service;

// Re-export main types
pub use adapters::InMemoryEventBus;
pub use domain::{
    Block, BlockHeader, ChainHead, ChainState, ConsensusAlgorithm, ConsensusConfig, ConsensusError,
    ConsensusResult, PBFTProof, PoSProof, SignedTransaction, ValidatedBlock, ValidationProof,
    ValidatorInfo, ValidatorSet,
};
pub use ipc::IpcHandler;
pub use ports::{ConsensusApi, EventBus, MempoolGateway, SignatureVerifier, ValidatorSetProvider};
pub use service::ConsensusService;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consensus_config_default() {
        let config = ConsensusConfig::default();
        assert_eq!(config.min_attestation_percent, 67);
        assert_eq!(config.max_block_gas, 30_000_000);
    }
}
