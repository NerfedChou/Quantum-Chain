//! Handler Layer
//!
//! Reference: Architecture.md - Hexagonal Architecture
//!
//! Contains IPC message handlers that validate and process
//! incoming messages per security rules.

pub mod ipc_handler;

pub use ipc_handler::BloomFilterHandler;
