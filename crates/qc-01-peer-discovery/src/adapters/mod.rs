//! # Adapters Layer
//!
//! Implements the V2.3 Choreography pattern for Peer Discovery.
//!
//! This module provides concrete implementations for the port traits.
//! Different adapters are feature-gated based on their dependencies.
//!
//! ## Feature Requirements
//!
//! | Adapter | Feature | Dependencies |
//! |---------|---------|--------------|
//! | `publisher` | `ipc` | shared-types (SecurityError) |
//! | `subscriber` | `ipc` | shared-types (SecurityError, SubsystemId) |
//! | `network` | (always) | None for pure types, `network` for tokio |
//! | `api_handler` | `rpc` | serde, serde_json |
//! | `bootstrap_handler` | `bootstrap` | uuid |

// =============================================================================
// NETWORK ADAPTERS (Pure Types Always Available)
// =============================================================================

/// Network adapters: SystemTimeSource, NoOpNetworkSocket, etc.
///
/// Pure types are always available. Tokio-based types require `network` feature.
pub mod network;

pub use network::{
    NoOpNetworkSocket, NoOpNodeIdValidator, ProofOfWorkValidator, StaticConfigProvider,
    SystemTimeSource,
};

#[cfg(feature = "network")]
pub use network::{ConfigError, MessageType, TomlConfigProvider, UdpNetworkSocket};

// =============================================================================
// IPC ADAPTERS (Requires `ipc` feature)
// =============================================================================

/// Event publisher for EDA integration.
/// Requires feature: `ipc` (depends on SecurityError)
#[cfg(feature = "ipc")]
pub mod publisher;

/// Event subscriber for EDA integration.
/// Requires feature: `ipc` (depends on SecurityError, SubsystemId)
#[cfg(feature = "ipc")]
pub mod subscriber;

#[cfg(feature = "ipc")]
pub use publisher::*;

#[cfg(feature = "ipc")]
pub use subscriber::*;

// =============================================================================
// RPC/API ADAPTERS (Requires `rpc` feature)
// =============================================================================

#[cfg(feature = "rpc")]
pub mod api_handler;

#[cfg(feature = "rpc")]
pub use api_handler::*;

// =============================================================================
// BOOTSTRAP HANDLER (Requires `bootstrap` feature)
// =============================================================================

#[cfg(feature = "bootstrap")]
pub mod bootstrap_handler;

#[cfg(feature = "bootstrap")]
pub use bootstrap_handler::*;

// =============================================================================
// SECURITY ADAPTERS (V2.5 - Always Available)
// =============================================================================

/// Security adapters: RandomSource, SecureHasher, RateLimiter implementations.
///
/// Provides both mock (for testing) and production implementations.
pub mod security;

pub use security::{
    FixedRandomSource, NoOpRateLimiter, OsRandomSource, SimpleHasher, SipHasher,
    SlidingWindowRateLimiter,
};

// =============================================================================
// FEELER NETWORK ADAPTER (Requires `network` feature)
// =============================================================================

/// Feeler network adapter for probing addresses.
///
/// Connects the pure domain `FeelerState` to actual network I/O.
#[cfg(feature = "network")]
pub mod feeler;

#[cfg(feature = "network")]
pub use feeler::{FeelerCoordinator, FeelerError, FeelerPort, MockFeelerPort};

#[cfg(feature = "quic")]
pub use feeler::QuicFeelerPort;
