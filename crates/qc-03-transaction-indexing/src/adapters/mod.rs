//! # Adapters Layer
//!
//! Secondary adapters for qc-03 Transaction Indexing subsystem.
//! These implement the hexagonal architecture pattern.

pub mod api_handler;

pub use api_handler::{handle_api_query, ApiGatewayHandler, ApiQueryError, Qc03Metrics};
