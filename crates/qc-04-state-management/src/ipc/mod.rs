//! # IPC Handler for State Management
//!
//! Provides authenticated message handling for direct IPC communication.
//! Uses centralized `MessageVerifier` from shared-types.
//!
//! ## Usage
//!
//! The IpcHandler is for direct request/response patterns. For event-based
//! choreography, use the node-runtime's StateAdapter instead.
//!
//! ## Security
//!
//! All handlers verify sender authorization per IPC-MATRIX.md before processing.

pub mod handler;

pub use handler::*;
