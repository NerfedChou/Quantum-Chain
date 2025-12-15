use super::port::{FeelerError, FeelerPort};
use crate::domain::{feeler::FeelerResult, handshake::ForkId, SocketAddr};
use std::time::Duration;

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
