//! # Adapters Security Module
//!
//! Subsystem-level security for all adapters.
//!
//! ## Modules
//!
//! - `validation`: Input validation utilities
//! - `rate_limit`: Rate limiting definitions

pub mod rate_limit;
#[cfg(test)]
mod tests;
mod validation;

pub use rate_limit::{RateLimitConfig, RateLimitResult};
pub use validation::{validate_batch_count, validate_block_height, validate_method_name};
