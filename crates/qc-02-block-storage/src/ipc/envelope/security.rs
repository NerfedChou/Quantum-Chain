//! # Envelope Security
//!
//! Security-related constants and utilities for envelope validation.
//!
//! ## Security Invariants
//!
//! - **HMAC Signature**: All messages authenticated via HMAC-SHA256
//! - **Replay Prevention**: Nonces cached and checked
//! - **Timestamp Bounds**: Messages must be recent (within MAX_MESSAGE_AGE_SECS)

/// Subsystem identifiers per IPC-MATRIX.md
pub mod subsystem_ids {
    pub const MEMPOOL: u8 = 1;
    pub const BLOCK_STORAGE: u8 = 2;
    pub const TRANSACTION_INDEXING: u8 = 3;
    pub const STATE_MANAGEMENT: u8 = 4;
    pub const SMART_CONTRACTS: u8 = 5;
    pub const PEER_ROUTING: u8 = 6;
    pub const BLOCK_PROPAGATION: u8 = 7;
    pub const CONSENSUS: u8 = 8;
    pub const FINALITY: u8 = 9;
    pub const SIGNATURE_VERIFICATION: u8 = 10;
    pub const LIGHT_CLIENTS: u8 = 11;
    pub const SYNC_PROTOCOL: u8 = 12;
    pub const RPC_GATEWAY: u8 = 13;
    pub const MONITORING: u8 = 14;
    pub const NODE_RUNTIME: u8 = 15;
    pub const API_GATEWAY: u8 = 16;
    pub const BLOCK_PRODUCTION: u8 = 17;
}

/// Maximum number of nonces to cache before cleanup
pub const MAX_NONCE_CACHE_SIZE: usize = 10000;

/// Nonce cleanup interval in seconds
pub const NONCE_CLEANUP_INTERVAL_SECS: u64 = 30;

/// Context for signature computation to reduce argument count
pub struct SignatureContext<'a> {
    pub shared_secret: &'a [u8; 32],
    pub version: u8,
    pub correlation_id: &'a [u8; 16],
    pub sender_id: u8,
    pub recipient_id: u8,
    pub timestamp: u64,
    pub nonce: u64,
}

/// Compute HMAC signature for message fields.
pub fn compute_message_signature(ctx: SignatureContext) -> [u8; 32] {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let mut mac =
        HmacSha256::new_from_slice(ctx.shared_secret).expect("HMAC key size is always valid");

    mac.update(&ctx.version.to_le_bytes());
    mac.update(ctx.correlation_id);
    mac.update(&[ctx.sender_id]);
    mac.update(&[ctx.recipient_id]);
    mac.update(&ctx.timestamp.to_le_bytes());
    mac.update(&ctx.nonce.to_le_bytes());

    let result = mac.finalize();
    let bytes = result.into_bytes();
    let mut sig = [0u8; 32];
    sig.copy_from_slice(&bytes);
    sig
}
