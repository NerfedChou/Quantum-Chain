//! # Adapters Layer - Event Bus Integration
//!
//! Publisher and subscriber adapters for the shared event bus.
//!
//! ## Publisher
//!
//! Publishes transaction batch proposals and status updates.
//!
//! ## Subscriber
//!
//! Receives storage confirmations and block rejection notifications
//! for Two-Phase Commit coordination.

pub mod publisher;
pub mod subscriber;

pub use publisher::{MempoolEventPublisher, NoOpPublisher, PublishError};
pub use subscriber::{MempoolEvent, MempoolEventSubscriber, NoOpSubscriber, SubscriptionHandle};

/// Combined topics module for all adapter topics.
pub mod topics {
    pub use super::publisher::topics as publish;
    pub use super::subscriber::topics as subscribe;
}
