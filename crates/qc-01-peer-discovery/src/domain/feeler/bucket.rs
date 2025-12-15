//! Bucket freshness tracking.

use std::collections::HashMap;

use crate::domain::Timestamp;

/// Bucket freshness tracking for stochastic selection
#[derive(Debug, Clone, Default)]
pub struct BucketFreshness {
    /// Last probe time per bucket
    last_probe: HashMap<usize, Timestamp>,
}

impl BucketFreshness {
    /// Create new bucket freshness tracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a bucket was probed
    pub fn record_probe(&mut self, bucket_idx: usize, now: Timestamp) {
        self.last_probe.insert(bucket_idx, now);
    }

    /// Select the stalest bucket (longest since last probe)
    ///
    /// If bucket_counts is provided, only considers buckets with entries
    pub fn select_stalest_bucket(&self, bucket_counts: &[usize], now: Timestamp) -> Option<usize> {
        let mut stalest_idx = None;
        let mut stalest_time = now.as_secs();

        for (idx, &count) in bucket_counts.iter().enumerate() {
            if count == 0 {
                continue;
            }

            let last = self.last_probe.get(&idx).map(|t| t.as_secs()).unwrap_or(0);
            if last < stalest_time {
                stalest_time = last;
                stalest_idx = Some(idx);
            }
        }

        stalest_idx
    }
}
