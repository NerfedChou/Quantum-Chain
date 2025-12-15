//! Tests for IPC Handler

use super::*;
use crate::ipc::security::SubsystemId;

#[test]
fn test_handler_new() {
    let handler = IpcHandler::new(&[0u8; 32]);
    assert_eq!(handler.subsystem_id(), SubsystemId::PeerDiscovery.as_u8());
    assert_eq!(handler.pending_request_count(), 0);
}

#[test]
fn test_pending_request_tracking() {
    let mut handler = IpcHandler::new(&[0u8; 32]);
    let correlation_id = [1u8; 16];
    let now = 1000u64;

    handler.register_pending_request(correlation_id, now);
    assert_eq!(handler.pending_request_count(), 1);

    // Correlation ID lookup removes the pending request (one-time use per Architecture.md 3.3)
    let matched = handler.match_response(&correlation_id);
    assert!(matched.is_some());
    assert_eq!(handler.pending_request_count(), 0);

    // Subsequent lookups return None - correlation IDs are single-use for replay prevention
    let matched_again = handler.match_response(&correlation_id);
    assert!(matched_again.is_none());
}

#[test]
fn test_gc_expired_requests() {
    let mut handler = IpcHandler::new(&[0u8; 32]);
    let correlation_id = [1u8; 16];
    let now = 1000u64;

    handler.register_pending_request(correlation_id, now);
    assert_eq!(handler.pending_request_count(), 1);

    // GC removes requests past their deadline (default 30s per Architecture.md 3.3)
    let expired_time = now + IpcHandler::<StaticKeyProvider>::DEFAULT_TIMEOUT_SECS + 1;
    let removed = handler.gc_expired_requests(expired_time);
    assert_eq!(removed, 1);
    assert_eq!(handler.pending_request_count(), 0);
}
