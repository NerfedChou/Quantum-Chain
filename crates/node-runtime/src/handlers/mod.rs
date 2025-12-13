//! # Event Handlers
//!
//! Message handlers for each subsystem that process choreography events.
//!
//! ## Plug-and-Play (v2.4)
//!
//! Handlers are conditionally compiled based on which subsystems are enabled.

#[cfg(feature = "qc-16")]
pub mod api_query;
#[cfg(feature = "qc-16")]
pub use api_query::ApiQueryHandler;

pub mod choreography;
pub use choreography::*;

// Re-export ConsensusHandler for easy access
#[cfg(feature = "qc-08")]
pub use choreography::ConsensusHandler;

#[cfg(feature = "qc-10")]
pub mod signature_verification;
#[cfg(feature = "qc-10")]
pub use signature_verification::SignatureVerificationHandler;
