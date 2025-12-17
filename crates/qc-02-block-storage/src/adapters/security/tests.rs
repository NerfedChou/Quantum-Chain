//! # Security Tests

use super::*;

#[test]
fn test_validate_block_height_valid() {
    assert!(validate_block_height(0).is_ok());
    assert!(validate_block_height(1_000_000).is_ok());
}

#[test]
fn test_validate_batch_count_capped() {
    assert_eq!(validate_batch_count(100), 100);
    assert_eq!(validate_batch_count(5000), 1000);
}

#[test]
fn test_validate_method_name() {
    assert!(validate_method_name("eth_blockNumber"));
    assert!(validate_method_name("debug_getMetrics"));
    assert!(validate_method_name("ping"));
    assert!(!validate_method_name("invalid<method>"));
    assert!(!validate_method_name("rm -rf /"));
}

#[test]
fn test_rate_limit_config_default() {
    let config = RateLimitConfig::default();
    assert_eq!(config.max_requests, 1000);
    assert!(config.per_ip);
}

#[test]
fn test_rate_limit_config_strict() {
    let config = RateLimitConfig::strict();
    assert_eq!(config.max_requests, 100);
}

#[test]
fn test_rate_limit_result() {
    assert!(RateLimitResult::Allowed.is_allowed());
    assert!(!RateLimitResult::Limited { retry_after: 30 }.is_allowed());
}
