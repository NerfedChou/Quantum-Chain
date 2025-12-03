//! Events and error types for Block Propagation subsystem.

use shared_types::Hash;
use thiserror::Error;

pub mod ipc;
pub mod p2p;

pub use ipc::*;
pub use p2p::*;

/// Block propagation errors.
#[derive(Debug, Error)]
pub enum PropagationError {
    #[error("Block already seen: {0:?}")]
    DuplicateBlock(Hash),

    #[error("Block too large: {size} bytes (max: {max})")]
    BlockTooLarge { size: usize, max: usize },

    #[error("Peer rate limited: {peer_id:?}")]
    RateLimited { peer_id: [u8; 32] },

    #[error("Unknown peer: {0:?}")]
    UnknownPeer([u8; 32]),

    #[error("Compact block reconstruction failed: missing {count} transactions")]
    ReconstructionFailed { count: usize },

    #[error("Request timeout for block: {0:?}")]
    Timeout(Hash),

    #[error("Unauthorized sender: subsystem {0}")]
    UnauthorizedSender(u8),

    #[error("Invalid block signature")]
    InvalidSignature,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("IPC security error: {0}")]
    IpcSecurityError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}
