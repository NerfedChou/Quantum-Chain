//! # Compaction Metrics (SPEC 4.3)
//!
//! Metrics for monitoring RocksDB/LSM compaction performance.
//!
//! ## Metrics Exported
//!
//! - Total compaction count
//! - Bytes compacted
//! - Compaction duration histogram
//! - Level sizes

use std::sync::atomic::{AtomicU64, Ordering};

// =============================================================================
// COMPACTION METRICS
// =============================================================================

/// Metrics for storage compaction operations
#[derive(Debug, Default)]
pub struct CompactionMetrics {
    /// Total number of compactions completed
    pub total_compactions: AtomicU64,
    /// Total bytes compacted
    pub bytes_compacted: AtomicU64,
    /// Total time spent in compaction (milliseconds)
    pub total_duration_ms: AtomicU64,
    /// Number of compactions currently in progress
    pub in_progress: AtomicU64,
}

impl CompactionMetrics {
    /// Create new metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a completed compaction
    pub fn record_compaction(&self, bytes: u64, duration_ms: u64) {
        self.total_compactions.fetch_add(1, Ordering::Relaxed);
        self.bytes_compacted.fetch_add(bytes, Ordering::Relaxed);
        self.total_duration_ms.fetch_add(duration_ms, Ordering::Relaxed);
    }

    /// Mark a compaction as started
    pub fn start_compaction(&self) {
        self.in_progress.fetch_add(1, Ordering::Relaxed);
    }

    /// Mark a compaction as finished
    pub fn finish_compaction(&self) {
        self.in_progress.fetch_sub(1, Ordering::Relaxed);
    }

    /// Get current compaction count
    pub fn compaction_count(&self) -> u64 {
        self.total_compactions.load(Ordering::Relaxed)
    }

    /// Get total bytes compacted
    pub fn bytes_total(&self) -> u64 {
        self.bytes_compacted.load(Ordering::Relaxed)
    }

    /// Get average compaction duration (ms)
    pub fn avg_duration_ms(&self) -> u64 {
        let count = self.compaction_count();
        if count == 0 {
            return 0;
        }
        self.total_duration_ms.load(Ordering::Relaxed) / count
    }

    /// Get compactions in progress
    pub fn in_progress_count(&self) -> u64 {
        self.in_progress.load(Ordering::Relaxed)
    }
}

// =============================================================================
// STORAGE METRICS
// =============================================================================

/// Combined storage metrics
#[derive(Debug, Default)]
pub struct StorageMetricsCollector {
    /// Compaction metrics
    pub compaction: CompactionMetrics,
    /// Total blocks stored
    pub blocks_stored: AtomicU64,
    /// Total bytes stored
    pub bytes_stored: AtomicU64,
    /// Read operations
    pub read_ops: AtomicU64,
    /// Write operations
    pub write_ops: AtomicU64,
}

impl StorageMetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a block write
    pub fn record_block_write(&self, size_bytes: u64) {
        self.blocks_stored.fetch_add(1, Ordering::Relaxed);
        self.bytes_stored.fetch_add(size_bytes, Ordering::Relaxed);
        self.write_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a read operation
    pub fn record_read(&self) {
        self.read_ops.fetch_add(1, Ordering::Relaxed);
    }

    /// Export as Prometheus-style metrics string
    pub fn export_prometheus(&self) -> String {
        format!(
            "# HELP qc02_blocks_stored Total blocks stored\n\
             # TYPE qc02_blocks_stored counter\n\
             qc02_blocks_stored {}\n\
             # HELP qc02_bytes_stored Total bytes stored\n\
             # TYPE qc02_bytes_stored counter\n\
             qc02_bytes_stored {}\n\
             # HELP qc02_read_ops Total read operations\n\
             # TYPE qc02_read_ops counter\n\
             qc02_read_ops {}\n\
             # HELP qc02_write_ops Total write operations\n\
             # TYPE qc02_write_ops counter\n\
             qc02_write_ops {}\n\
             # HELP qc02_compactions Total compactions\n\
             # TYPE qc02_compactions counter\n\
             qc02_compactions {}\n",
            self.blocks_stored.load(Ordering::Relaxed),
            self.bytes_stored.load(Ordering::Relaxed),
            self.read_ops.load(Ordering::Relaxed),
            self.write_ops.load(Ordering::Relaxed),
            self.compaction.compaction_count(),
        )
    }
}

// =============================================================================
// TESTS (TDD)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compaction_metrics_new() {
        let metrics = CompactionMetrics::new();
        assert_eq!(metrics.compaction_count(), 0);
        assert_eq!(metrics.bytes_total(), 0);
        assert_eq!(metrics.in_progress_count(), 0);
    }

    #[test]
    fn test_compaction_record() {
        let metrics = CompactionMetrics::new();
        
        metrics.record_compaction(1000, 50);
        metrics.record_compaction(2000, 100);
        
        assert_eq!(metrics.compaction_count(), 2);
        assert_eq!(metrics.bytes_total(), 3000);
        assert_eq!(metrics.avg_duration_ms(), 75);
    }

    #[test]
    fn test_compaction_in_progress() {
        let metrics = CompactionMetrics::new();
        
        metrics.start_compaction();
        metrics.start_compaction();
        assert_eq!(metrics.in_progress_count(), 2);
        
        metrics.finish_compaction();
        assert_eq!(metrics.in_progress_count(), 1);
    }

    #[test]
    fn test_storage_metrics_block_write() {
        let collector = StorageMetricsCollector::new();
        
        collector.record_block_write(1024);
        collector.record_block_write(2048);
        
        assert_eq!(collector.blocks_stored.load(Ordering::Relaxed), 2);
        assert_eq!(collector.bytes_stored.load(Ordering::Relaxed), 3072);
        assert_eq!(collector.write_ops.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_storage_metrics_reads() {
        let collector = StorageMetricsCollector::new();
        
        collector.record_read();
        collector.record_read();
        collector.record_read();
        
        assert_eq!(collector.read_ops.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn test_prometheus_export() {
        let collector = StorageMetricsCollector::new();
        collector.record_block_write(100);
        collector.record_read();
        
        let output = collector.export_prometheus();
        
        assert!(output.contains("qc02_blocks_stored 1"));
        assert!(output.contains("qc02_bytes_stored 100"));
        assert!(output.contains("qc02_read_ops 1"));
    }
}
