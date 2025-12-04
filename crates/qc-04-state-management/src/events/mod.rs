//! # Event Payloads for State Management
//!
//! Defines message payloads for IPC communication per IPC-MATRIX.md.
//!
//! ## Choreography Events
//!
//! - `BlockValidatedPayload`: Received from Consensus (8)
//! - `StateRootComputedPayload`: Published to Block Storage (2)
//!
//! ## Request/Response Payloads
//!
//! - `StateReadRequestPayload`: From (6, 11, 12, 14)
//! - `StateWriteRequestPayload`: From (11) only
//! - `BalanceCheckRequestPayload`: From (6) only
//! - `ConflictDetectionRequestPayload`: From (12) only

pub mod payloads;

pub use payloads::*;
