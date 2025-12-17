use crate::domain::errors::FSError;
use crate::ports::outbound::FileSystemAdapter;

/// Controllable filesystem adapter for unit tests.
///
/// Allows tests to simulate disk space conditions for INVARIANT-2 verification.
/// Production uses `ProductionFileSystemAdapter` in node-runtime.
pub struct MockFileSystemAdapter {
    available_percent: u8,
}

impl MockFileSystemAdapter {
    /// Create adapter reporting `available_percent` disk space.
    pub fn new(available_percent: u8) -> Self {
        Self { available_percent }
    }

    /// Update reported disk space for test scenarios.
    pub fn set_available_percent(&mut self, percent: u8) {
        self.available_percent = percent;
    }
}

impl FileSystemAdapter for MockFileSystemAdapter {
    fn available_disk_space_percent(&self) -> Result<u8, FSError> {
        Ok(self.available_percent)
    }

    fn available_disk_space_bytes(&self) -> Result<u64, FSError> {
        // Assume 1TB total, return proportional
        Ok((1_000_000_000_000u64 * self.available_percent as u64) / 100)
    }

    fn total_disk_space_bytes(&self) -> Result<u64, FSError> {
        Ok(1_000_000_000_000) // 1TB
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_filesystem() {
        let mut fs = MockFileSystemAdapter::new(50);

        assert_eq!(fs.available_disk_space_percent().unwrap(), 50);

        fs.set_available_percent(4);
        assert_eq!(fs.available_disk_space_percent().unwrap(), 4);
    }
}
