//! WebSocket module for real-time subscriptions.

mod client;

pub use client::{BlockHeader, WsClient, WsEvent};
