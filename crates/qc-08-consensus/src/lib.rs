//! # qc-08-consensus
//!
//! Consensus subsystem for Quantum-Chain.
//!
//! ## Architecture
//!
//! This subsystem implements block validation and agreement using either
//! Proof of Stake (PoS) with 2/3 attestation threshold or PBFT with 2f+1 votes.
//!
//! ### V2.3 Choreography Pattern
//!
//! Consensus performs validation ONLY - it does NOT orchestrate block storage.
//! After validating a block, it publishes `BlockValidated` to the Event Bus,
//! which triggers the choreography:
//!
//! ```text
//! Consensus (8) ──BlockValidated──→ [Event Bus]
//!                                        │
//!                  ┌─────────────────────┼─────────────────────┐
//!                  ↓                     ↓                     ↓
//!         [Tx Indexing (3)]    [State Mgmt (4)]    [Block Storage (2)]
//! ```
//!
//! ### Zero-Trust Signature Verification
//!
//! Per IPC-MATRIX.md, Consensus MUST NOT trust pre-validation flags.
//! All signatures are independently re-verified before processing.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use qc_08_consensus::{ConsensusService, ConsensusConfig};
//! use qc_08_consensus::ports::{EventBus, MempoolGateway, SignatureVerifier, ValidatorSetProvider};
//!
//! let service = ConsensusService::new(
//!     event_bus,
//!     mempool,
//!     sig_verifier,
//!     validator_provider,
//!     ConsensusConfig::default(),
//! );
//!
//! // Validate a block
//! let validated = service.validate_block(block, None).await?;
//! ```
//!
//! ## Security
//!
//! - All IPC messages validated via shared MessageVerifier
//! - HMAC signature verification on all incoming messages
//! - Nonce-based replay protection
//! - Timestamp validation to prevent stale message attacks
//! - Sender authorization per IPC-MATRIX.md

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
