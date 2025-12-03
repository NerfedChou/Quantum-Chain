//! Adapters layer for the Mempool subsystem.
//!
//! Provides event bus integration for inter-subsystem communication.

pub mod publisher;
pub mod subscriber;

pub use publisher::{MempoolEventPublisher, NoOpPublisher, PublishError};
pub use subscriber::{MempoolEvent, MempoolEventSubscriber, NoOpSubscriber, SubscriptionHandle};

/// Combined topics module for all adapter topics.
pub mod topics {
    pub use super::publisher::topics as publish;
    pub use super::subscriber::topics as subscribe;
}
