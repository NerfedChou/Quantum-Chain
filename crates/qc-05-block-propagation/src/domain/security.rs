//! # Header-First Propagation (Pipelining)
//!
//! Split propagation into Header and Body phases for DoS protection.
//!
//! ## Problem
//!
//! Processing full 10MB blocks before PoW validation wastes bandwidth.
//!
//! ## Solution: Header-First Validation
//!
//! 1. Receive 80-byte header
//! 2. Validate PoW, timestamp, parent
//! 3. Only then request body
//!
//! ## Algorithm
//!
//! ```text
//! Header (80B) → Validate PoW → Request Body → Stream Download
//! ```

use shared_types::Hash;
use std::time::{Duration, Instant};

/// Propagation phase for header-first validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropagationPhase {
    /// Waiting for header
    WaitingHeader,
    /// Header received, validating PoW
    ValidatingHeader,
    /// Header valid, requesting body
    RequestingBody,
    /// Body streaming in progress
    StreamingBody,
    /// Block complete
    Complete,
    /// Header failed validation
    HeaderInvalid,
    /// Download stalled
    Stalled,
}

/// Header-first block download state.
#[derive(Clone, Debug)]
pub struct HeaderFirstDownload {
    pub block_hash: Hash,
    pub phase: PropagationPhase,
    pub header_received: Option<Instant>,
    pub body_requested: Option<Instant>,
    pub body_completed: Option<Instant>,
    pub bytes_received: usize,
    pub expected_size: usize,
}

impl HeaderFirstDownload {
    pub fn new(block_hash: Hash) -> Self {
        Self {
            block_hash,
            phase: PropagationPhase::WaitingHeader,
            header_received: None,
            body_requested: None,
            body_completed: None,
            bytes_received: 0,
            expected_size: 0,
        }
    }

    /// Record header received and start validation.
    pub fn on_header_received(&mut self) {
        self.header_received = Some(Instant::now());
        self.phase = PropagationPhase::ValidatingHeader;
    }

    /// Mark header as valid and request body.
    pub fn on_header_valid(&mut self, expected_size: usize) {
        self.expected_size = expected_size;
        self.body_requested = Some(Instant::now());
        self.phase = PropagationPhase::RequestingBody;
    }

    /// Mark header as invalid (PoW failure).
    pub fn on_header_invalid(&mut self) {
        self.phase = PropagationPhase::HeaderInvalid;
    }

    /// Update streaming progress.
    pub fn on_bytes_received(&mut self, bytes: usize) {
        self.bytes_received += bytes;
        self.phase = PropagationPhase::StreamingBody;
    }

    /// Mark block as complete.
    pub fn on_complete(&mut self) {
        self.body_completed = Some(Instant::now());
        self.phase = PropagationPhase::Complete;
    }

    /// Check if download has stalled.
    pub fn check_stalled(&mut self, timeout: Duration) -> bool {
        if let Some(requested) = self.body_requested {
            if requested.elapsed() > timeout && self.phase != PropagationPhase::Complete {
                self.phase = PropagationPhase::Stalled;
                return true;
            }
        }
        false
    }

    /// Calculate download progress (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.expected_size == 0 {
            0.0
        } else {
            (self.bytes_received as f64 / self.expected_size as f64).min(1.0)
        }
    }

    /// Get total download time.
    pub fn total_time(&self) -> Option<Duration> {
        match (self.header_received, self.body_completed) {
            (Some(start), Some(end)) => Some(end.duration_since(start)),
            _ => None,
        }
    }
}

// =============================================================================
// STALLING PROTECTION
// =============================================================================

/// Stalling protection with sliding window timeout.
///
/// ## Threat
///
/// Malicious peer announces block but delivers at 1 byte/sec,
/// holding download slot open.
///
/// ## Defense
///
/// Track average throughput, set dynamic timeout = 3 × expected.
#[derive(Clone, Debug)]
pub struct StallingTracker {
    /// Recent download times (sliding window)
    recent_times: Vec<Duration>,
    /// Window size
    window_size: usize,
    /// Multiplier for timeout (default: 3x)
    timeout_multiplier: f64,
}

/// Default timeout multiplier.
pub const DEFAULT_TIMEOUT_MULTIPLIER: f64 = 3.0;

/// Default sliding window size.
pub const STALLING_WINDOW_SIZE: usize = 5;

impl StallingTracker {
    pub fn new() -> Self {
        Self {
            recent_times: Vec::with_capacity(STALLING_WINDOW_SIZE),
            window_size: STALLING_WINDOW_SIZE,
            timeout_multiplier: DEFAULT_TIMEOUT_MULTIPLIER,
        }
    }

    /// Record a successful download time.
    pub fn record_download(&mut self, time: Duration) {
        if self.recent_times.len() >= self.window_size {
            self.recent_times.remove(0);
        }
        self.recent_times.push(time);
    }

    /// Get average download time.
    pub fn average_time(&self) -> Duration {
        if self.recent_times.is_empty() {
            Duration::from_secs(10) // Default 10s
        } else {
            let total: Duration = self.recent_times.iter().sum();
            total / self.recent_times.len() as u32
        }
    }

    /// Calculate expected download time for a given size.
    pub fn expected_time(&self, block_size: usize) -> Duration {
        let avg = self.average_time();
        // Scale by size (assuming avg is for ~1MB blocks)
        let scale = (block_size as f64 / 1_000_000.0).max(1.0);
        Duration::from_secs_f64(avg.as_secs_f64() * scale)
    }

    /// Get timeout threshold (3 × expected).
    pub fn timeout_threshold(&self, block_size: usize) -> Duration {
        let expected = self.expected_time(block_size);
        Duration::from_secs_f64(expected.as_secs_f64() * self.timeout_multiplier)
    }

    /// Check if a download is stalling.
    pub fn is_stalling(&self, block_size: usize, elapsed: Duration) -> bool {
        elapsed > self.timeout_threshold(block_size)
    }
}

impl Default for StallingTracker {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// UNSOLICITED BLOCK FILTER
// =============================================================================

/// Unsolicited block filter with request-response matching.
///
/// ## Threat
///
/// Attacker bypasses header-first and sends 10MB block messages.
///
/// ## Defense
///
/// Track pending requests, filter unmatched blocks.
use std::collections::HashMap;

/// Pending block request.
#[derive(Clone, Debug)]
pub struct PendingRequest {
    pub block_hash: Hash,
    pub requested_at: Instant,
    pub nonce: u64,
}

/// Unsolicited block filter.
pub struct UnsolicitedBlockFilter {
    /// Pending requests: nonce -> request
    pending: HashMap<u64, PendingRequest>,
    /// Request timeout
    timeout: Duration,
    /// Next nonce
    next_nonce: u64,
}

/// Result of checking an incoming block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockFilterResult {
    /// Block matches a pending request
    Allowed { nonce: u64 },
    /// Block is unsolicited but possibly new announcement
    PossibleAnnouncement,
    /// Block is spam (duplicate/old)
    Spam,
}

impl UnsolicitedBlockFilter {
    pub fn new(timeout: Duration) -> Self {
        Self {
            pending: HashMap::new(),
            timeout,
            next_nonce: 0,
        }
    }

    /// Register a block request.
    pub fn register_request(&mut self, block_hash: Hash) -> u64 {
        let nonce = self.next_nonce;
        self.next_nonce += 1;
        
        self.pending.insert(nonce, PendingRequest {
            block_hash,
            requested_at: Instant::now(),
            nonce,
        });
        
        nonce
    }

    /// Check if incoming block was requested.
    pub fn check_block(&mut self, block_hash: &Hash, seen_before: bool) -> BlockFilterResult {
        // Clean expired requests
        self.clean_expired();

        // Check if this matches a pending request
        for (nonce, req) in &self.pending {
            if req.block_hash == *block_hash {
                return BlockFilterResult::Allowed { nonce: *nonce };
            }
        }

        // Not requested - check if it's a new announcement
        if seen_before {
            BlockFilterResult::Spam
        } else {
            BlockFilterResult::PossibleAnnouncement
        }
    }

    /// Complete a request.
    pub fn complete_request(&mut self, nonce: u64) {
        self.pending.remove(&nonce);
    }

    /// Clean expired requests.
    fn clean_expired(&mut self) {
        let timeout = self.timeout;
        self.pending.retain(|_, req| req.requested_at.elapsed() < timeout);
    }

    /// Get pending count.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_propagation_phase_transitions() {
        let mut download = HeaderFirstDownload::new([0xAB; 32]);
        assert_eq!(download.phase, PropagationPhase::WaitingHeader);
        
        download.on_header_received();
        assert_eq!(download.phase, PropagationPhase::ValidatingHeader);
        
        download.on_header_valid(1_000_000);
        assert_eq!(download.phase, PropagationPhase::RequestingBody);
    }

    #[test]
    fn test_header_invalid() {
        let mut download = HeaderFirstDownload::new([0xAB; 32]);
        download.on_header_received();
        download.on_header_invalid();
        assert_eq!(download.phase, PropagationPhase::HeaderInvalid);
    }

    #[test]
    fn test_download_progress() {
        let mut download = HeaderFirstDownload::new([0xAB; 32]);
        download.on_header_received();
        download.on_header_valid(1000);
        
        download.on_bytes_received(500);
        assert_eq!(download.progress(), 0.5);
        
        download.on_bytes_received(500);
        assert_eq!(download.progress(), 1.0);
    }

    #[test]
    fn test_stalling_tracker_average() {
        let mut tracker = StallingTracker::new();
        
        tracker.record_download(Duration::from_secs(2));
        tracker.record_download(Duration::from_secs(4));
        tracker.record_download(Duration::from_secs(6));
        
        assert_eq!(tracker.average_time(), Duration::from_secs(4));
    }

    #[test]
    fn test_stalling_threshold() {
        let mut tracker = StallingTracker::new();
        tracker.record_download(Duration::from_secs(2));
        
        let threshold = tracker.timeout_threshold(1_000_000);
        // 3 × 2s = 6s for 1MB
        assert!(threshold >= Duration::from_secs(5));
    }

    #[test]
    fn test_stalling_detection() {
        let tracker = StallingTracker::new();
        
        // Default 10s × 3 = 30s timeout
        assert!(!tracker.is_stalling(1_000_000, Duration::from_secs(20)));
        assert!(tracker.is_stalling(1_000_000, Duration::from_secs(40)));
    }

    #[test]
    fn test_unsolicited_filter_allowed() {
        let mut filter = UnsolicitedBlockFilter::new(Duration::from_secs(60));
        let hash = [0xAB; 32];
        
        let nonce = filter.register_request(hash);
        
        let result = filter.check_block(&hash, false);
        assert!(matches!(result, BlockFilterResult::Allowed { .. }));
        
        filter.complete_request(nonce);
        assert_eq!(filter.pending_count(), 0);
    }

    #[test]
    fn test_unsolicited_filter_spam() {
        let mut filter = UnsolicitedBlockFilter::new(Duration::from_secs(60));
        let hash = [0xAB; 32];
        
        // Block not requested and already seen
        let result = filter.check_block(&hash, true);
        assert_eq!(result, BlockFilterResult::Spam);
    }

    #[test]
    fn test_unsolicited_filter_announcement() {
        let mut filter = UnsolicitedBlockFilter::new(Duration::from_secs(60));
        let hash = [0xAB; 32];
        
        // Block not requested but never seen (new announcement)
        let result = filter.check_block(&hash, false);
        assert_eq!(result, BlockFilterResult::PossibleAnnouncement);
    }
}
