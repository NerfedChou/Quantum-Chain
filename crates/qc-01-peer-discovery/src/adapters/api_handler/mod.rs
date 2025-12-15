//! # API Gateway Request Handler
//!
//! Handles requests from qc-16 (API Gateway) for admin_peers, admin_nodeInfo, etc.
//!
//! ## Supported Methods
//!
//! - `get_peers` - Returns connected peers for admin_peers RPC
//! - `get_node_info` - Returns node info for admin_nodeInfo RPC
//! - `add_peer` - Adds a peer (admin_addPeer)
//! - `remove_peer` - Removes a peer (admin_removePeer)
//! - `get_subsystem_metrics` - Returns qc-01 specific metrics for debug panel
//! - `ping` - Health check

// Semantic submodules
mod routes;
mod types;

// Re-export public API
pub use routes::{handle_api_query, ApiGatewayHandler};
pub use types::*;

#[cfg(test)]
mod tests;
