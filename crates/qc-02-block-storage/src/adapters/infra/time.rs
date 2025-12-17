use crate::domain::storage::Timestamp;
use crate::ports::outbound::TimeSource;

/// Default time source using system time.
#[derive(Default)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}
