//! Metrics collection for block production subsystem

use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics collector for block production
#[derive(Debug, Default)]
pub struct Metrics {
    /// Total blocks produced
    pub blocks_produced: AtomicU64,

    /// Total transactions included
    pub transactions_included: AtomicU64,

    /// Total gas used across all blocks
    pub total_gas_used: AtomicU64,

    /// Total fees collected
    pub total_fees_collected: AtomicU64,

    /// Total transaction selection time (microseconds)
    pub selection_time_us: AtomicU64,

    /// Total PoW mining time (milliseconds)
    pub mining_time_ms: AtomicU64,

    /// Total MEV bundles detected
    pub mev_bundles_detected: AtomicU64,
}

impl Metrics {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a produced block
    pub fn record_block_produced(&self, tx_count: u32, gas_used: u64) {
        self.blocks_produced.fetch_add(1, Ordering::Relaxed);
        self.transactions_included
            .fetch_add(tx_count as u64, Ordering::Relaxed);
        self.total_gas_used.fetch_add(gas_used, Ordering::Relaxed);
    }

    /// Record transaction selection time
    pub fn record_selection_time(&self, duration_us: u64) {
        self.selection_time_us
            .fetch_add(duration_us, Ordering::Relaxed);
    }

    /// Record PoW mining time
    pub fn record_mining_time(&self, duration_ms: u64) {
        self.mining_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Record MEV bundle detection
    pub fn record_mev_bundle(&self) {
        self.mev_bundles_detected.fetch_add(1, Ordering::Relaxed);
    }

    /// Get blocks produced
    pub fn get_blocks_produced(&self) -> u64 {
        self.blocks_produced.load(Ordering::Relaxed)
    }

    /// Get average transactions per block
    pub fn get_avg_transactions_per_block(&self) -> f64 {
        let blocks = self.blocks_produced.load(Ordering::Relaxed);
        if blocks == 0 {
            return 0.0;
        }
        let txs = self.transactions_included.load(Ordering::Relaxed);
        txs as f64 / blocks as f64
    }

    /// Get average gas per block
    pub fn get_avg_gas_per_block(&self) -> f64 {
        let blocks = self.blocks_produced.load(Ordering::Relaxed);
        if blocks == 0 {
            return 0.0;
        }
        let gas = self.total_gas_used.load(Ordering::Relaxed);
        gas as f64 / blocks as f64
    }

    /// Get average selection time (microseconds)
    pub fn get_avg_selection_time(&self) -> f64 {
        let blocks = self.blocks_produced.load(Ordering::Relaxed);
        if blocks == 0 {
            return 0.0;
        }
        let time = self.selection_time_us.load(Ordering::Relaxed);
        time as f64 / blocks as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_recording() {
        let metrics = Metrics::new();

        metrics.record_block_produced(100, 15_000_000);
        metrics.record_block_produced(150, 20_000_000);

        assert_eq!(metrics.get_blocks_produced(), 2);
        assert_eq!(metrics.get_avg_transactions_per_block(), 125.0);
        assert_eq!(metrics.get_avg_gas_per_block(), 17_500_000.0);
    }

    #[test]
    fn test_selection_time() {
        let metrics = Metrics::new();

        metrics.record_block_produced(100, 15_000_000);
        metrics.record_selection_time(5000);

        assert_eq!(metrics.get_avg_selection_time(), 5000.0);
    }
}
