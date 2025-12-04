//! Ports module for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 3

pub mod inbound;
pub mod outbound;

pub use inbound::FinalityApi;
pub use outbound::{AttestationVerifier, BlockStorageGateway, ValidatorSetProvider};
