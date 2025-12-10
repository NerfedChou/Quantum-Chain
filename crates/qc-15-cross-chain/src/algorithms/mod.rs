//! # Algorithms Module
//!
//! Core algorithms for Cross-Chain Communication.
//!
//! Reference: System.md Lines 736-739

pub mod secret;
pub mod atomic_swap;

pub use secret::{generate_random_secret, create_hash_lock, verify_secret, verify_claim, verify_refund};
pub use atomic_swap::{create_atomic_swap, validate_swap_timelocks, calculate_timelocks, is_swap_complete, is_swap_refunded};
