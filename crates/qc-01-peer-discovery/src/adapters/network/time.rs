use crate::domain::Timestamp;
use crate::ports::TimeSource;

// ============================================================================
// SystemTimeSource - Production Time Source
// ============================================================================

/// Production time source using the system clock.
///
/// This adapter implements `TimeSource` using `std::time::SystemTime`.
/// For testing, use the `ControllableTimeSource` from the test utilities.
///
/// # Example
///
/// ```rust
/// use qc_01_peer_discovery::adapters::network::{SystemTimeSource};
/// use qc_01_peer_discovery::ports::TimeSource;
///
/// let time_source = SystemTimeSource::new();
/// let now = time_source.now();
/// assert!(now.as_secs() > 0);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeSource;

impl SystemTimeSource {
    /// Create a new system time source.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        use std::time::{SystemTime, UNIX_EPOCH};

        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        Timestamp::new(duration.as_secs())
    }
}
