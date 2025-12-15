//! Feeler service configuration.

/// Feeler service configuration
#[derive(Debug, Clone)]
pub struct FeelerConfig {
    /// Base interval between feeler probes (seconds)
    pub probe_interval_secs: u64,
    /// Maximum random jitter added to interval (seconds)
    pub jitter_max_secs: u64,
    /// Connection timeout for feeler probe (seconds)
    pub connection_timeout_secs: u64,
    /// Maximum failures before address is removed from New table
    pub max_failures: u32,
    /// Maximum active feeler connections at once
    pub max_concurrent_probes: usize,
}

impl Default for FeelerConfig {
    fn default() -> Self {
        Self {
            probe_interval_secs: 120,    // 2 minutes
            jitter_max_secs: 30,         // 0-30s jitter
            connection_timeout_secs: 10, // 10 second timeout
            max_failures: 3,
            max_concurrent_probes: 2,
        }
    }
}

impl FeelerConfig {
    /// Testing config with faster intervals
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            probe_interval_secs: 5,
            jitter_max_secs: 2,
            connection_timeout_secs: 2,
            max_failures: 2,
            max_concurrent_probes: 1,
        }
    }
}
