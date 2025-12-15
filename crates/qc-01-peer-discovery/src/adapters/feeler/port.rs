use crate::domain::{feeler::FeelerResult, handshake::ForkId, SocketAddr};
use std::time::Duration;

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
