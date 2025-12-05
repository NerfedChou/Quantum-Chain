//! API client module for communicating with qc-16 Admin endpoints.
//!
//! Uses JSON-RPC to call debug_* methods on the admin server.

mod client;
mod types;

pub use client::AdminApiClient;
pub use types::*;
