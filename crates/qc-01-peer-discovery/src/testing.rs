//! Centralized Testing Utilities
//!
//! This module collects all test helpers, mocks, and fixtures used across the crate.
//! It is available with the `test-utils` feature flag.

use crate::domain::Timestamp;
use crate::ports::outbound::TimeSource;

// Re-export specific mocks from adapters if available
#[cfg(feature = "network")]
// Or whatever feature gates adapters::security if any, lib.rs says it's always available but let's check imports
pub use crate::adapters::FixedRandomSource;

/// A time source that returns a fixed timestamp.
///
/// Useful for deterministic testing where time progression needs to be controlled.
///
/// # Example
///
/// ```rust
/// use qc_01_peer_discovery::testing::FixedTimeSource;
/// use qc_01_peer_discovery::TimeSource;
///
/// let time = FixedTimeSource::new(12345);
/// assert_eq!(time.now().as_secs(), 12345);
/// ```
#[derive(Debug, Clone)]
pub struct FixedTimeSource {
    timestamp: u64,
}

impl FixedTimeSource {
    /// Create a new fixed time source with the given timestamp (in seconds).
    pub fn new(timestamp: u64) -> Self {
        Self { timestamp }
    }

    /// Get the configured timestamp value.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

impl TimeSource for FixedTimeSource {
    fn now(&self) -> Timestamp {
        Timestamp::new(self.timestamp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_time_source_returns_configured_value() {
        let source = FixedTimeSource::new(1000);
        assert_eq!(source.now().as_secs(), 1000);
    }

    #[test]
    fn test_fixed_time_source_is_clone() {
        let source = FixedTimeSource::new(500);
        let cloned = source.clone();
        assert_eq!(source.now().as_secs(), cloned.now().as_secs());
    }
}
