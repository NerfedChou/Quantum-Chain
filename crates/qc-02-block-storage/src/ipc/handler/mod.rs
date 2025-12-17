//! # Handler Module
//!
//! BlockStorageHandler for IPC message handling.
//!
//! ## Modules
//!
//! - `core`: BlockStorageHandler struct and methods
//! - `helpers`: Shared utilities
//! - `types`: HandlerError and error conversions
//! - `security`: Handler authorization constants

mod core;
mod helpers;
mod security;
#[cfg(test)]
mod tests;
mod types;

pub use core::BlockStorageHandler;
pub use types::HandlerError;
