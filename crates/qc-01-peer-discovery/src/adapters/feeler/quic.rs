use super::port::{FeelerError, FeelerPort};
use crate::domain::{feeler::FeelerResult, handshake::ForkId, SocketAddr};
use std::time::Duration;

#[cfg(feature = "quic")]
use crate::transport::quic::QuicTransport;

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
