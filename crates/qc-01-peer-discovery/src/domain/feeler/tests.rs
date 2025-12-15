//! Tests for Feeler Connection Service
//!
//! Reference: Bitcoin Core's `-feeler` connection logic

use super::*;
use crate::domain::{IpAddr, SocketAddr, Timestamp};

fn make_socket(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::v4(192, 168, 1, 1), port)
}

// =============================================================================
// TEST GROUP 1: Probe Scheduling
// =============================================================================

#[test]
fn test_should_probe_after_interval() {
    let config = FeelerConfig::for_testing();
    let now = Timestamp::new(1000);
    let state = FeelerState::new(config.clone(), now);

    // Should not probe immediately
    assert!(!state.should_probe(now));

    // Should probe after interval
    let later = Timestamp::new(now.as_secs() + config.probe_interval_secs);
    assert!(state.should_probe(later));
}

#[test]
fn test_probe_respects_max_concurrent() {
    let config = FeelerConfig::for_testing();
    let now = Timestamp::new(1000);
    let mut state = FeelerState::new(config.clone(), now);

    // Start first probe
    let target1 = make_socket(8080);
    assert!(state.start_probe(target1, None, now).is_some());

    // Can't start another (max_concurrent_probes = 1)
    let target2 = make_socket(8081);
    assert!(state.start_probe(target2, None, now).is_none());
}

// =============================================================================
// TEST GROUP 2: Probe Completion
// =============================================================================

#[test]
fn test_probe_success_resets_failures() {
    let config = FeelerConfig::for_testing();
    let now = Timestamp::new(1000);
    let mut state = FeelerState::new(config, now);

    let target = make_socket(8080);

    // Simulate some failures
    state.start_probe(target, None, now);
    state.on_probe_failure(&target);
    assert_eq!(state.failure_count(&target), 1);

    // Success resets
    state.start_probe(target, None, now);
    state.on_probe_success(&target);
    assert_eq!(state.failure_count(&target), 0);
}

#[test]
fn test_max_failures_triggers_removal() {
    let config = FeelerConfig::for_testing();
    let now = Timestamp::new(1000);
    let mut state = FeelerState::new(config.clone(), now);

    let target = make_socket(8080);

    // First failure
    state.start_probe(target, None, now);
    assert!(!state.on_probe_failure(&target));

    // Second failure (max_failures = 2) - should trigger removal
    state.start_probe(target, None, now);
    assert!(state.on_probe_failure(&target));
}

// =============================================================================
// TEST GROUP 3: Timeout Handling
// =============================================================================

#[test]
fn test_probe_timeout_detection() {
    let config = FeelerConfig::for_testing();
    let now = Timestamp::new(1000);
    let mut state = FeelerState::new(config.clone(), now);

    let target = make_socket(8080);
    state.start_probe(target, None, now);

    // Not timed out yet
    assert!(state.get_timed_out_probes(now).is_empty());

    // After timeout
    let later = Timestamp::new(now.as_secs() + config.connection_timeout_secs + 1);
    let timed_out = state.get_timed_out_probes(later);
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out[0], target);
}

// =============================================================================
// TEST GROUP 4: Bucket Selection
// =============================================================================

#[test]
fn test_stalest_bucket_selection() {
    let now = Timestamp::new(1000);
    let mut freshness = BucketFreshness::new();

    // Bucket 0 probed at 500, bucket 1 probed at 800, bucket 2 never probed
    freshness.record_probe(0, Timestamp::new(500));
    freshness.record_probe(1, Timestamp::new(800));

    // Bucket counts: all have entries
    let counts = vec![5, 3, 7];

    // Should select bucket 2 (never probed = stalest)
    let selected = freshness.select_stalest_bucket(&counts, now);
    assert_eq!(selected, Some(2));
}

#[test]
fn test_stalest_bucket_skips_empty() {
    let now = Timestamp::new(1000);
    let freshness = BucketFreshness::new();

    // Bucket 0 and 2 have entries, bucket 1 is empty
    let counts = vec![5, 0, 7];

    let selected = freshness.select_stalest_bucket(&counts, now);
    // Should select 0 or 2, not 1
    assert!(selected == Some(0) || selected == Some(2));
}
