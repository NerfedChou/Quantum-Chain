//! Transport layer modules.
//!
//! ## Available Transports
//!
//! - `quic` - QUIC/HTTP3 with encrypted headers and 0-RTT support
//!
//! ## Feature Gates
//!
//! - `quic` feature: Enables full async QUIC transport with quinn
//! - Without feature: Basic replay protection and config types only

pub mod quic;

pub use quic::{QuicConfig, QuicConnectionState, QuicError, QuicTransport, ReplayProtection};
