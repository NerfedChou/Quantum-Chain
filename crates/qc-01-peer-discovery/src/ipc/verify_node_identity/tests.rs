//! Tests for Verify Node Identity

use super::*;

#[test]
fn test_verify_node_identity_request_new() {
    let node_id = [1u8; 32];
    let pubkey = [2u8; 33];
    let sig = [3u8; 64];

    let req = VerifyNodeIdentityRequest::new(node_id, pubkey, sig);

    assert_eq!(req.node_id, node_id);
    assert_eq!(req.claimed_pubkey, pubkey);
    assert_eq!(req.signature, sig);
}

#[test]
fn test_verify_node_identity_request_clone() {
    let req = VerifyNodeIdentityRequest::new([1u8; 32], [2u8; 33], [3u8; 64]);
    let cloned = req.clone();
    assert_eq!(req, cloned);
}
