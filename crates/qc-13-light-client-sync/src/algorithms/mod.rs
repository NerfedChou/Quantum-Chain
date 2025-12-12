//! # Algorithms Module
//!
//! Core SPV algorithms for Light Client Sync.
//!
//! Reference: System.md Lines 627-630

pub mod header_sync;
pub mod merkle_verifier;
pub mod multi_node;

pub use header_sync::{append_headers_batch, find_common_ancestor, validate_header_batch};
pub use merkle_verifier::{build_merkle_proof, compute_merkle_root, verify_merkle_proof};
pub use multi_node::{check_consensus, check_strict_consensus, required_for_consensus};
