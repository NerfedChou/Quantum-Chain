//! Metrics and tracing hooks for Bloom filter operations
//!
//! Provides instrumentation points for monitoring filter performance,
//! resource usage, and operation latencies.
//!
//! ## Usage
//!
//! ```ignore
//! use qc_07_bloom_filters::metrics::{Metrics, MetricsRecorder};
//!
//! // Create a metrics recorder
//! let metrics = Metrics::new();
//!
//! // Record filter creation
//! metrics.record_filter_created(1000, 7, 50);
//!
//! // Record lookup
//! let start = std::time::Instant::now();
//! let result = filter.contains(element);
//! metrics.record_lookup(start.elapsed(), result);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Metrics collector for Bloom filter operations
///
/// Thread-safe counters and gauges for monitoring filter performance.
#[derive(Default)]
pub struct Metrics {
    /// Total filters created
    pub filters_created: AtomicU64,
    /// Total elements inserted across all filters
    pub elements_inserted: AtomicU64,
    /// Total lookups performed
    pub lookups_performed: AtomicU64,
    /// Total positive lookups (matches)
    pub lookups_positive: AtomicU64,
    /// Total filter merges
    pub filters_merged: AtomicU64,
    /// Total bytes allocated for filters
    pub bytes_allocated: AtomicU64,
    /// Cumulative lookup time in nanoseconds
    pub lookup_time_ns: AtomicU64,
    /// Cumulative insert time in nanoseconds
    pub insert_time_ns: AtomicU64,
}

impl Metrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self::default()
    }

    /// Record filter creation
    ///
    /// # Arguments
    /// * `size_bits` - Filter size in bits
    /// * `hash_count` - Number of hash functions (k)
    /// * `expected_elements` - Expected number of elements
    pub fn record_filter_created(&self, size_bits: usize, _hash_count: usize, _expected_elements: usize) {
        self.filters_created.fetch_add(1, Ordering::Relaxed);
        // Convert bits to bytes for allocation tracking
        self.bytes_allocated.fetch_add((size_bits / 8) as u64, Ordering::Relaxed);
    }

    /// Record element insertion
    ///
    /// # Arguments
    /// * `duration` - Time taken for insertion
    pub fn record_insert(&self, duration: Duration) {
        self.elements_inserted.fetch_add(1, Ordering::Relaxed);
        self.insert_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record lookup operation
    ///
    /// # Arguments
    /// * `duration` - Time taken for lookup
    /// * `found` - Whether the element was found (possibly false positive)
    pub fn record_lookup(&self, duration: Duration, found: bool) {
        self.lookups_performed.fetch_add(1, Ordering::Relaxed);
        self.lookup_time_ns.fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
        if found {
            self.lookups_positive.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record filter merge operation
    pub fn record_merge(&self) {
        self.filters_merged.fetch_add(1, Ordering::Relaxed);
    }

    /// Record filter deallocation
    ///
    /// # Arguments
    /// * `size_bits` - Filter size in bits being freed
    pub fn record_filter_freed(&self, size_bits: usize) {
        self.bytes_allocated.fetch_sub((size_bits / 8) as u64, Ordering::Relaxed);
    }

    /// Get current metrics snapshot
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            filters_created: self.filters_created.load(Ordering::Relaxed),
            elements_inserted: self.elements_inserted.load(Ordering::Relaxed),
            lookups_performed: self.lookups_performed.load(Ordering::Relaxed),
            lookups_positive: self.lookups_positive.load(Ordering::Relaxed),
            filters_merged: self.filters_merged.load(Ordering::Relaxed),
            bytes_allocated: self.bytes_allocated.load(Ordering::Relaxed),
            avg_lookup_ns: self.avg_lookup_time_ns(),
            avg_insert_ns: self.avg_insert_time_ns(),
        }
    }

    /// Calculate average lookup time in nanoseconds
    pub fn avg_lookup_time_ns(&self) -> u64 {
        let total = self.lookup_time_ns.load(Ordering::Relaxed);
        let count = self.lookups_performed.load(Ordering::Relaxed);
        if count > 0 {
            total / count
        } else {
            0
        }
    }

    /// Calculate average insert time in nanoseconds
    pub fn avg_insert_time_ns(&self) -> u64 {
        let total = self.insert_time_ns.load(Ordering::Relaxed);
        let count = self.elements_inserted.load(Ordering::Relaxed);
        if count > 0 {
            total / count
        } else {
            0
        }
    }

    /// Calculate observed false positive rate
    ///
    /// This is the ratio of positive lookups to total lookups.
    /// Note: This includes both true positives and false positives.
    pub fn observed_positive_rate(&self) -> f64 {
        let total = self.lookups_performed.load(Ordering::Relaxed);
        let positive = self.lookups_positive.load(Ordering::Relaxed);
        if total > 0 {
            positive as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Reset all counters
    pub fn reset(&self) {
        self.filters_created.store(0, Ordering::Relaxed);
        self.elements_inserted.store(0, Ordering::Relaxed);
        self.lookups_performed.store(0, Ordering::Relaxed);
        self.lookups_positive.store(0, Ordering::Relaxed);
        self.filters_merged.store(0, Ordering::Relaxed);
        self.bytes_allocated.store(0, Ordering::Relaxed);
        self.lookup_time_ns.store(0, Ordering::Relaxed);
        self.insert_time_ns.store(0, Ordering::Relaxed);
    }
}

/// Point-in-time metrics snapshot
#[derive(Clone, Debug, Default)]
pub struct MetricsSnapshot {
    pub filters_created: u64,
    pub elements_inserted: u64,
    pub lookups_performed: u64,
    pub lookups_positive: u64,
    pub filters_merged: u64,
    pub bytes_allocated: u64,
    pub avg_lookup_ns: u64,
    pub avg_insert_ns: u64,
}

/// Trait for custom metrics recording implementations
///
/// Implement this trait to integrate with external metrics systems
/// like Prometheus, StatsD, or OpenTelemetry.
pub trait MetricsRecorder: Send + Sync {
    /// Record filter creation
    fn record_filter_created(&self, size_bits: usize, hash_count: usize, expected_elements: usize);

    /// Record element insertion
    fn record_insert(&self, duration: Duration);

    /// Record lookup operation
    fn record_lookup(&self, duration: Duration, found: bool);

    /// Record filter merge
    fn record_merge(&self);

    /// Record filter deallocation
    fn record_filter_freed(&self, size_bits: usize);
}

/// No-op metrics recorder for when metrics are disabled
#[derive(Default)]
pub struct NoOpMetrics;

impl MetricsRecorder for NoOpMetrics {
    fn record_filter_created(&self, _: usize, _: usize, _: usize) {}
    fn record_insert(&self, _: Duration) {}
    fn record_lookup(&self, _: Duration, _: bool) {}
    fn record_merge(&self) {}
    fn record_filter_freed(&self, _: usize) {}
}

impl MetricsRecorder for Metrics {
    fn record_filter_created(&self, size_bits: usize, hash_count: usize, expected_elements: usize) {
        Metrics::record_filter_created(self, size_bits, hash_count, expected_elements);
    }

    fn record_insert(&self, duration: Duration) {
        Metrics::record_insert(self, duration);
    }

    fn record_lookup(&self, duration: Duration, found: bool) {
        Metrics::record_lookup(self, duration, found);
    }

    fn record_merge(&self) {
        Metrics::record_merge(self);
    }

    fn record_filter_freed(&self, size_bits: usize) {
        Metrics::record_filter_freed(self, size_bits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = Metrics::new();
        let snapshot = metrics.snapshot();

        assert_eq!(snapshot.filters_created, 0);
        assert_eq!(snapshot.elements_inserted, 0);
        assert_eq!(snapshot.lookups_performed, 0);
    }

    #[test]
    fn test_record_filter_created() {
        let metrics = Metrics::new();

        metrics.record_filter_created(1000, 7, 50);
        metrics.record_filter_created(2000, 10, 100);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.filters_created, 2);
        assert_eq!(snapshot.bytes_allocated, 375); // (1000 + 2000) / 8
    }

    #[test]
    fn test_record_lookups() {
        let metrics = Metrics::new();

        metrics.record_lookup(Duration::from_nanos(100), true);
        metrics.record_lookup(Duration::from_nanos(150), false);
        metrics.record_lookup(Duration::from_nanos(120), true);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.lookups_performed, 3);
        assert_eq!(snapshot.lookups_positive, 2);
        assert_eq!(snapshot.avg_lookup_ns, 123); // (100 + 150 + 120) / 3
    }

    #[test]
    fn test_observed_positive_rate() {
        let metrics = Metrics::new();

        for _ in 0..100 {
            metrics.record_lookup(Duration::from_nanos(100), false);
        }
        for _ in 0..10 {
            metrics.record_lookup(Duration::from_nanos(100), true);
        }

        let rate = metrics.observed_positive_rate();
        assert!((rate - 0.0909).abs() < 0.01); // 10/110 ≈ 0.0909
    }

    #[test]
    fn test_reset() {
        let metrics = Metrics::new();

        metrics.record_filter_created(1000, 7, 50);
        metrics.record_lookup(Duration::from_nanos(100), true);
        metrics.record_insert(Duration::from_nanos(50));

        metrics.reset();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.filters_created, 0);
        assert_eq!(snapshot.lookups_performed, 0);
        assert_eq!(snapshot.elements_inserted, 0);
    }

    #[test]
    fn test_noop_metrics() {
        // Just verify NoOpMetrics compiles and doesn't panic
        let metrics = NoOpMetrics;
        metrics.record_filter_created(1000, 7, 50);
        metrics.record_insert(Duration::from_nanos(100));
        metrics.record_lookup(Duration::from_nanos(100), true);
        metrics.record_merge();
        metrics.record_filter_freed(1000);
    }
}
