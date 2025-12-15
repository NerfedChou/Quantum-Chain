//! Tests for QUIC Transport Layer

use super::*;

#[test]
fn test_default_config() {
    let config = QuicConfig::default();
    assert_eq!(config.max_streams, 100);
    assert!(config.enable_0rtt);
}

#[test]
fn test_replay_protection_fresh_token() {
    let mut rp = ReplayProtection::new(Duration::from_secs(60));
    let token = [1u8; 32];

    assert!(rp.check_token(&token));
}

#[test]
fn test_replay_protection_rejects_duplicate() {
    let mut rp = ReplayProtection::new(Duration::from_secs(60));
    let token = [2u8; 32];

    assert!(rp.check_token(&token));
    assert!(!rp.check_token(&token)); // Replay rejected
}

#[test]
fn test_replay_protection_different_tokens() {
    let mut rp = ReplayProtection::new(Duration::from_secs(60));

    assert!(rp.check_token(&[1u8; 32]));
    assert!(rp.check_token(&[2u8; 32]));
    assert!(rp.check_token(&[3u8; 32]));
}

#[test]
fn test_quic_transport_0rtt() {
    let mut transport = QuicTransport::new(QuicConfig::default());
    let token = [0xAB; 32];

    assert!(transport.check_0rtt_token(&token));
    assert!(!transport.check_0rtt_token(&token));
}

#[test]
fn test_connection_state_creation() {
    let state = QuicConnectionState::new("127.0.0.1:8443".parse().unwrap(), [0u8; 16]);
    assert!(!state.established);
    assert_eq!(state.bytes_sent, 0);
    assert_eq!(state.bytes_received, 0);
}

#[test]
fn test_quic_error_display() {
    let err = QuicError::ConnectionTimeout {
        remote: "127.0.0.1:8443".into(),
    };
    assert!(err.to_string().contains("timed out"));
}
