//! Outbound ports for the API Gateway.

use async_trait::async_trait;

/// Time source trait for testability
#[async_trait]
pub trait TimeSource: Send + Sync {
    fn now(&self) -> u64;
}

/// System time implementation
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|_| {
                // Clock before Unix epoch - return 0 rather than panic
                // This should never happen in practice
                0
            })
    }
}

impl Default for SystemTimeSource {
    fn default() -> Self {
        Self
    }
}
