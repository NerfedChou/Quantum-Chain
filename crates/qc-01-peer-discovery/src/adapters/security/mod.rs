//! # Security Adapters (V2.5)
//!
//! Implements the security port traits for hardening:
//! - `RandomSource`: Cryptographically secure random number generation
//! - `SecureHasher`: DoS-resistant keyed hashing
//! - `RateLimiter`: Domain-level rate limiting backstop
//!
//! ## Mock vs Production
//!
//! | Adapter | Mock (Testing) | Production |
//! |---------|----------------|------------|
//! | `RandomSource` | `FixedRandomSource` | `OsRandomSource` |
//! | `SecureHasher` | `SimpleHasher` | `SipHasher` |
//! | `RateLimiter` | `NoOpRateLimiter` | `SlidingWindowRateLimiter` |

mod hashing;
mod random;
mod rate_limit;

// Re-export public types
pub use hashing::{SimpleHasher, SipHasher};
pub use random::{FixedRandomSource, OsRandomSource};
pub use rate_limit::{NoOpRateLimiter, SlidingWindowRateLimiter};

// Note: Trait imports needed for module code are in submodules

#[cfg(test)]
mod tests;
