//! # Error Types
//!
//! Defines error types used across subsystems.

use thiserror::Error;

/// Errors that can occur in the Block Storage subsystem.
#[derive(Debug, Clone, Error)]
pub enum StorageError {
    /// Block not found in storage.
    #[error("Block not found: {0}")]
    NotFound(String),
    
    /// Data corruption detected during read.
    #[error("Data corruption: checksum mismatch for block {block_hash}")]
    DataCorruption { block_hash: String },
    
    /// Disk space below required threshold.
    #[error("Disk full: only {available_percent}% available, need 5%")]
    DiskFull { available_percent: u8 },
    
    /// Parent block not found (INVARIANT-1 violation).
    #[error("Parent block not found: cannot write block at height {height}")]
    ParentNotFound { height: u64 },
    
    /// Database operation failed.
    #[error("Database error: {0}")]
    DatabaseError(String),
}

/// Errors related to message verification.
#[derive(Debug, Clone, Error)]
pub enum MessageError {
    /// Message version not supported.
    #[error("Unsupported version: received {received}, supported {supported}")]
    UnsupportedVersion { received: u16, supported: u16 },
    
    /// Timestamp outside valid window.
    #[error("Timestamp out of range: {timestamp} not within valid window")]
    TimestampOutOfRange { timestamp: u64 },
    
    /// Replay attack detected.
    #[error("Replay detected: nonce {nonce} already seen")]
    ReplayDetected { nonce: String },
    
    /// Invalid signature.
    #[error("Invalid signature")]
    InvalidSignature,
    
    /// Reply-to field mismatch (forwarding attack).
    #[error("Reply-to mismatch: reply_to.subsystem_id={reply_to} != sender_id={sender}")]
    ReplyToMismatch { reply_to: u8, sender: u8 },
    
    /// Unauthorized sender for this message type.
    #[error("Unauthorized: subsystem {sender} not allowed to send {message_type}")]
    Unauthorized { sender: u8, message_type: String },
}

/// Node operational states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Normal operation.
    Running,
    /// Synchronizing with the network.
    Syncing,
    /// Halted due to repeated sync failures (awaiting intervention).
    HaltedAwaitingIntervention,
}
