//! # Feeler Network Adapter
//!
//! Connects the pure domain `FeelerState` to actual network I/O.
//!
//! ## Architecture (Hexagonal)
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │                   Application Layer                   │
//! │  ┌────────────────────────────────────────────────┐  │
//! │  │              FeelerCoordinator                 │  │
//! │  │    (orchestrates probing lifecycle)            │  │
//! │  └────────────────────────────────────────────────┘  │
//! │         ▲                       ▲                     │
//! │         │                       │                     │
//! │  ┌──────┴──────┐         ┌──────┴──────┐             │
//! │  │ FeelerState │         │AddressManager│             │
//! │  │  (domain)   │         │  (domain)   │             │
//! │  └─────────────┘         └─────────────┘             │
//! └──────────────────────────────────────────────────────┘
//!         │
//!   ┌─────┴─────┐
//!   │ Port:     │
//!   │FeelerPort │
//!   └─────┬─────┘
//!         │
//! ┌───────┴────────┐
//! │    Adapter:    │
//! │ QuicFeeler     │
//! └───────┬────────┘
//!         │
//!   ┌─────┴─────┐
//!   │ QUIC      │
//!   │ Transport │
//!   └───────────┘
//! ```

use std::time::Duration;

#[cfg(feature = "quic")]
use crate::transport::quic::QuicTransport;

use crate::domain::{
    feeler::{BucketFreshness, FeelerConfig, FeelerResult, FeelerState},
    handshake::ForkId,
    NodeId, SocketAddr,
};
use crate::ports::TimeSource;

// =============================================================================
// FEELER PORT (Driven Port)
// =============================================================================

/// Port for feeler probe network operations.
///
/// This port abstracts the network I/O required for feeler probing,
/// allowing the domain to remain pure while adapters handle actual connections.
pub trait FeelerPort: Send + Sync {
    /// Probe a peer address.
    ///
    /// # Arguments
    ///
    /// * `addr` - Address to probe
    /// * `timeout` - Maximum time to wait for response
    /// * `our_fork_id` - Our ForkId for chain compatibility check
    ///
    /// # Returns
    ///
    /// - `Ok(FeelerResult::Success)` if peer is reachable and compatible
    /// - `Ok(FeelerResult::ConnectionFailed)` if peer unreachable
    /// - `Ok(FeelerResult::WrongChain)` if ForkId mismatch
    /// - `Err` on internal error
    fn probe(
        &self,
        addr: &SocketAddr,
        timeout: Duration,
        our_fork_id: &ForkId,
    ) -> Result<FeelerResult, FeelerError>;
}

/// Errors that can occur during feeler operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeelerError {
    /// Transport not initialized.
    NotInitialized,
    /// Network I/O error.
    NetworkError {
        /// Error description.
        reason: String,
    },
}

impl std::fmt::Display for FeelerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "feeler transport not initialized"),
            Self::NetworkError { reason } => write!(f, "network error: {}", reason),
        }
    }
}

impl std::error::Error for FeelerError {}

// =============================================================================
// FEELER COORDINATOR (Application Service)
// =============================================================================

/// Coordinates feeler probing between domain state and network adapter.
///
/// This is the application-layer service that ties together:
/// - `FeelerState` (domain logic)
/// - `FeelerPort` (network adapter)
/// - `AddressManager` (peer address source)
pub struct FeelerCoordinator<T: TimeSource, P: FeelerPort> {
    /// Domain state
    state: FeelerState,
    /// Bucket freshness tracker
    freshness: BucketFreshness,
    /// Network adapter
    port: P,
    /// Time source
    time_source: T,
    /// Our ForkId for compatibility checks
    our_fork_id: ForkId,
}

impl<T: TimeSource, P: FeelerPort> FeelerCoordinator<T, P> {
    /// Create a new feeler coordinator.
    pub fn new(config: FeelerConfig, port: P, time_source: T, fork_id: ForkId) -> Self {
        let now = time_source.now();
        Self {
            state: FeelerState::new(config, now),
            freshness: BucketFreshness::new(),
            port,
            time_source,
            our_fork_id: fork_id,
        }
    }

    /// Check if it's time to probe and execute if needed.
    ///
    /// # Arguments
    ///
    /// * `bucket_counts` - Number of addresses in each New table bucket
    /// * `get_address` - Closure to get a random address from a bucket
    ///
    /// # Returns
    ///
    /// Result of probe if one was executed, None otherwise.
    pub fn maybe_probe<F>(
        &mut self,
        bucket_counts: &[usize],
        get_address: F,
    ) -> Option<(SocketAddr, FeelerResult)>
    where
        F: FnOnce(usize) -> Option<(SocketAddr, Option<NodeId>)>,
    {
        let now = self.time_source.now();

        if !self.state.should_probe(now) {
            return None;
        }

        // Select stalest bucket
        let bucket_idx = self.freshness.select_stalest_bucket(bucket_counts, now)?;

        // Get address from that bucket
        let (target_addr, node_id) = get_address(bucket_idx)?;

        // Start probe in domain
        let probe = self.state.start_probe(target_addr, node_id, now)?;

        // Record bucket probe
        self.freshness.record_probe(bucket_idx, now);

        // Execute probe via network adapter
        let timeout = Duration::from_secs(probe.deadline.as_secs() - now.as_secs());
        let result = self
            .port
            .probe(&target_addr, timeout, &self.our_fork_id)
            .unwrap_or(FeelerResult::ConnectionFailed);

        // Update domain state
        match &result {
            FeelerResult::Success => self.state.on_probe_success(&target_addr),
            _ => {
                let should_remove = self.state.on_probe_failure(&target_addr);
                if should_remove {
                    // Caller should remove from New table
                }
            }
        }

        Some((target_addr, result))
    }

    /// Check for timed-out probes and mark them as failed.
    pub fn gc_timed_out(&mut self) -> Vec<SocketAddr> {
        let now = self.time_source.now();
        let timed_out = self.state.get_timed_out_probes(now);

        for addr in &timed_out {
            self.state.cancel_probe(addr);
            self.state.on_probe_failure(addr);
        }

        timed_out
    }

    /// Get number of active probes.
    pub fn active_probe_count(&self) -> usize {
        self.state.active_probe_count()
    }
}

// =============================================================================
// MOCK FEELER ADAPTER (for testing)
// =============================================================================

/// Mock feeler adapter for testing.
#[derive(Debug, Default)]
pub struct MockFeelerPort {
    /// Pre-configured results for each address.
    results: std::collections::HashMap<SocketAddr, FeelerResult>,
}

impl MockFeelerPort {
    /// Create a new mock port.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure result for an address.
    pub fn set_result(&mut self, addr: SocketAddr, result: FeelerResult) {
        self.results.insert(addr, result);
    }
}

impl FeelerPort for MockFeelerPort {
    fn probe(
        &self,
        addr: &SocketAddr,
        _timeout: Duration,
        _our_fork_id: &ForkId,
    ) -> Result<FeelerResult, FeelerError> {
        Ok(self
            .results
            .get(addr)
            .cloned()
            .unwrap_or(FeelerResult::ConnectionFailed))
    }
}

// =============================================================================
// QUIC FEELER ADAPTER (production)
// =============================================================================

/// Production feeler adapter using QUIC transport.
#[cfg(feature = "quic")]
pub struct QuicFeelerPort {
    /// Shared QUIC transport (owned by network layer)
    /// In production this would be Arc<Mutex<QuicTransport>>
    /// For now we keep it simple with Option
    _marker: std::marker::PhantomData<QuicTransport>,
}

#[cfg(feature = "quic")]
impl QuicFeelerPort {
    /// Create a new QUIC feeler port.
    ///
    /// Note: In production, this would take an Arc<Mutex<QuicTransport>>
    /// shared with the main network layer.
    pub fn new() -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(feature = "quic")]
impl Default for QuicFeelerPort {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "quic")]
impl FeelerPort for QuicFeelerPort {
    fn probe(
        &self,
        _addr: &SocketAddr,
        _timeout: Duration,
        _our_fork_id: &ForkId,
    ) -> Result<FeelerResult, FeelerError> {
        // TODO: Implement actual QUIC probe
        // 1. Connect to peer with timeout
        // 2. Send STATUS message with our ForkId
        // 3. Validate their ForkId response
        // 4. Close connection
        //
        // For now, return success as placeholder
        // This will be wired when async runtime integration is complete
        Ok(FeelerResult::Success)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests;
