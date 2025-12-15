//! Connection slots configuration.

/// Connection slots configuration
#[derive(Debug, Clone)]
pub struct ConnectionSlotsConfig {
    /// Maximum outbound connections (sacred, never filled by inbound)
    pub max_outbound: usize,
    /// Maximum inbound connections
    pub max_inbound: usize,
    /// Minimum uptime (seconds) to be "protected" from eviction
    pub protection_threshold_secs: u64,
    /// Minimum score to be "protected" from eviction
    pub protection_threshold_score: f64,
    /// Maximum peers protected per eviction round
    pub max_protected_per_round: usize,
}

impl Default for ConnectionSlotsConfig {
    fn default() -> Self {
        Self {
            max_outbound: 10,
            max_inbound: 40,
            protection_threshold_secs: 3600,
            protection_threshold_score: 5.0,
            max_protected_per_round: 10,
        }
    }
}

impl ConnectionSlotsConfig {
    /// Testing config with smaller limits
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            max_outbound: 3,
            max_inbound: 5,
            protection_threshold_secs: 60,
            protection_threshold_score: 2.0,
            max_protected_per_round: 2,
        }
    }
}
