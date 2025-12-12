//! # Algorithms Module
//!
//! Core algorithms for Cross-Chain Communication.
//!
//! Reference: System.md Lines 736-739

pub mod atomic_swap;
pub mod secret;

pub use atomic_swap::{
    calculate_timelocks, create_atomic_swap, is_swap_complete, is_swap_refunded,
    validate_swap_timelocks,
};
pub use secret::{
    create_hash_lock, generate_random_secret, verify_claim, verify_refund, verify_secret,
};
