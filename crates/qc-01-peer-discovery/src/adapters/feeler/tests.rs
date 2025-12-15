//! Tests for Feeler Adapter
use super::*;
use crate::domain::IpAddr;
use crate::testing::FixedTimeSource;

fn make_socket(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::v4(192, 168, 1, 1), port)
}

#[test]
fn test_mock_feeler_port() {
    let mut port = MockFeelerPort::new();
    let addr = make_socket(8080);
    port.set_result(addr, FeelerResult::Success);

    let fork_id = ForkId::new(0, 0);
    let result = port.probe(&addr, Duration::from_secs(5), &fork_id);

    assert_eq!(result.unwrap(), FeelerResult::Success);
}

#[test]
fn test_feeler_coordinator_probe_scheduling() {
    let config = FeelerConfig::default();
    let port = MockFeelerPort::new();
    let time_source = FixedTimeSource::new(1000);
    let fork_id = ForkId::new(0, 0);

    let coordinator = FeelerCoordinator::new(config.clone(), port, time_source, fork_id);

    // Should not probe immediately
    assert_eq!(coordinator.active_probe_count(), 0);
}

#[test]
fn test_feeler_error_display() {
    let err = FeelerError::NetworkError {
        reason: "connection refused".into(),
    };
    assert!(err.to_string().contains("connection refused"));
}
