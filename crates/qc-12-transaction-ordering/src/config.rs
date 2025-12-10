//! Configuration for Transaction Ordering Subsystem

use serde::{Deserialize, Serialize};

/// Ordering configuration
/// Reference: SPEC-12 Section 7, Lines 540-545
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderingConfig {
    /// Maximum transactions to analyze at once
    pub max_batch_size: usize,
    /// Maximum edges in dependency graph (anti-DoS)
    pub max_edge_count: usize,
    /// Fallback to sequential if conflicts exceed threshold
    pub conflict_threshold_percent: u8,
    /// Enable speculative execution
    pub enable_speculation: bool,
    /// Timestamp window for replay prevention (seconds)
    pub timestamp_window_secs: u64,
}

impl Default for OrderingConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            max_edge_count: 10_000,
            conflict_threshold_percent: 50,
            enable_speculation: true,
            timestamp_window_secs: 120,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrderingConfig::default();
        assert_eq!(config.max_batch_size, 1000);
        assert_eq!(config.max_edge_count, 10_000);
        assert_eq!(config.conflict_threshold_percent, 50);
        assert!(config.enable_speculation);
    }
}
