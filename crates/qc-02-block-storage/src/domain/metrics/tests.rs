//! # Metrics Tests

use super::*;
use std::sync::atomic::Ordering;

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
