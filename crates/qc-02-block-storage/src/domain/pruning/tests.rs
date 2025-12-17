//! # Pruning Tests

use super::*;

#[test]
fn test_pruning_config_default() {
    let config = PruningConfig::default();
    assert_eq!(config.keep_recent, 10_000);
    assert_eq!(config.anchor_base, 1000);
    assert!(config.keep_headers);
    assert!(!config.enabled);
}

#[test]
fn test_is_anchor_genesis() {
    let svc = PruningService::new(PruningConfig::default());
    assert!(svc.is_anchor_block(0));
}

#[test]
fn test_is_anchor_at_base_intervals() {
    let config = PruningConfig {
        anchor_base: 1000,
        ..Default::default()
    };
    let svc = PruningService::new(config);

    assert!(svc.is_anchor_block(1000));
    assert!(svc.is_anchor_block(2000));
    assert!(svc.is_anchor_block(4000));
}

#[test]
fn test_is_not_anchor() {
    let svc = PruningService::new(PruningConfig::default());

    assert!(!svc.is_anchor_block(500));
    assert!(!svc.is_anchor_block(1001));
    assert!(!svc.is_anchor_block(1500));
}

#[test]
fn test_should_prune_keeps_recent() {
    let config = PruningConfig {
        keep_recent: 100,
        enabled: true,
        ..Default::default()
    };
    let svc = PruningService::new(config);

    assert!(!svc.should_prune(950, 1000));
    assert!(!svc.should_prune(901, 1000));
}

#[test]
fn test_should_prune_old_non_anchor() {
    let config = PruningConfig {
        keep_recent: 100,
        anchor_base: 1000,
        enabled: true,
        ..Default::default()
    };
    let svc = PruningService::new(config);

    assert!(svc.should_prune(500, 20000));
}

#[test]
fn test_get_prunable_heights() {
    let config = PruningConfig {
        keep_recent: 100,
        anchor_base: 100,
        enabled: true,
        keep_headers: true,
    };
    let svc = PruningService::new(config);

    let prunable = svc.get_prunable_heights(1, 50, 1000);
    assert!(!prunable.contains(&0));
}

#[test]
fn test_disabled_pruning() {
    let config = PruningConfig {
        enabled: false,
        ..Default::default()
    };
    let svc = PruningService::new(config);

    assert!(!svc.should_prune(1, 100000));
}
