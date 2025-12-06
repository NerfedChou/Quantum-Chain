//! # Shared Bus - Event Bus for Inter-Subsystem Communication
//!
//! Implements the V2.3 Choreography Pattern as mandated by Architecture.md.
//!
//! ## Architecture Rules (Architecture.md Section 5)
//!
//! - **RULE #4:** All inter-subsystem communication via Shared Bus ONLY
//! - **Direct calls between subsystems are FORBIDDEN**
//! - All messages wrapped in `AuthenticatedMessage<T>` envelope
//!
//! ## Choreography Pattern
//!
//! ```text
//! ┌──────────────┐                    ┌──────────────┐
//! │ Subsystem A  │                    │ Subsystem B  │
//! │              │    publish()       │              │
//! │              │ ──────┐            │              │
//! └──────────────┘       │            └──────────────┘
//!                        ▼                    ↑
//!                  ┌──────────────┐          │
//!                  │  Event Bus   │          │
//!                  │              │ ─────────┘
//!                  └──────────────┘  subscribe()
//! ```
//!
//! ## Security
//!
//! - **Time-Bounded Nonce Cache:** Prevents replay attacks (v2.1)
//! - **Envelope-Only Identity:** `sender_id` from envelope is sole authority
//! - **Dead Letter Queue:** Failed messages routed to DLQ for investigation

// Nursery lints that are too strict
#![allow(clippy::missing_const_for_fn)]
// Allow in tests
#![cfg_attr(test, allow(clippy::unwrap_used))]
#![cfg_attr(test, allow(clippy::expect_used))]
#![cfg_attr(test, allow(clippy::panic))]

pub mod events;
pub mod nonce_cache;
pub mod publisher;
pub mod subscriber;

// Re-export main types
pub use events::{ApiQueryError, BlockchainEvent, EventFilter, EventTopic};
pub use nonce_cache::TimeBoundedNonceCache;
pub use publisher::{EventPublisher, InMemoryEventBus};
pub use subscriber::{EventStream, EventSubscriber, Subscription, SubscriptionError};

/// Current protocol version for event bus messages.
pub const PROTOCOL_VERSION: u16 = 1;

/// Maximum events to buffer per subscriber before backpressure.
pub const DEFAULT_CHANNEL_CAPACITY: usize = 1000;

/// Dead Letter Queue topic for failed messages.
pub const DLQ_TOPIC: &str = "dlq.critical";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_default_capacity() {
        assert_eq!(DEFAULT_CHANNEL_CAPACITY, 1000);
    }
}
