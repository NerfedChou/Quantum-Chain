//! WebSocket module for real-time subscriptions.
//!
//! Per SPEC-16 Section 5, supports:
//! - eth_subscribe / eth_unsubscribe
//! - Subscription types: newHeads, logs, newPendingTransactions, syncing
//! - Message size limits and rate limiting

pub mod handler;
pub mod subscriptions;

pub use handler::{
    WebSocketConfig, WebSocketHandler, DEFAULT_MAX_MESSAGE_SIZE, DEFAULT_RATE_LIMIT,
};
pub use subscriptions::{SubscribeError, SubscriptionManager, SubscriptionNotification};
