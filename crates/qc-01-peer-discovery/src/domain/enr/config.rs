//! ENR configuration.

/// ENR configuration
#[derive(Debug, Clone)]
pub struct EnrConfig {
    /// Maximum size of an ENR record (bytes)
    pub max_record_size: usize,
    /// Maximum age of a record before considered stale (seconds)
    pub max_record_age_secs: u64,
    /// Maximum capabilities per record
    pub max_capabilities: usize,
}

impl Default for EnrConfig {
    fn default() -> Self {
        Self {
            max_record_size: 300,
            max_record_age_secs: 86400,
            max_capabilities: 16,
        }
    }
}
