//! Tests for IPC Bootstrap

use super::*;
use super::*;

fn make_request() -> BootstrapRequest {
    BootstrapRequest::new(BootstrapRequestConfig {
        node_id: [1u8; 32],
        ip_address: IpAddr::v4(192, 168, 1, 100),
        port: 8080,
        proof_of_work: [0u8; 32], // PoW with leading zeros
        claimed_pubkey: [2u8; 33],
        signature: [3u8; 64],
    })
}

#[test]
fn test_bootstrap_request_new() {
    let req = make_request();
    assert_eq!(req.node_id, [1u8; 32]);
    assert_eq!(req.port, 8080);
}

#[test]
fn test_to_verification_request() {
    let req = make_request();
    let verify_req = req.to_verification_request();

    assert_eq!(verify_req.node_id, req.node_id);
    assert_eq!(verify_req.claimed_pubkey, req.claimed_pubkey);
    assert_eq!(verify_req.signature, req.signature);
}

#[test]
fn test_bootstrap_request_clone() {
    let req = make_request();
    let cloned = req.clone();
    assert_eq!(req, cloned);
}
