use super::traits::{PeerDiscoveryEventPublisher, VerificationRequestPublisher};
use crate::ipc::payloads::{PeerDiscoveryEventPayload, PeerListResponsePayload};
use crate::ipc::VerifyNodeIdentityRequest;

/// No-op publisher for testing.
#[derive(Debug, Default)]
pub struct NoOpEventPublisher {
    /// Count of published events (for testing verification).
    pub event_count: std::sync::atomic::AtomicUsize,
}

impl NoOpEventPublisher {
    /// Create a new no-op publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            event_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get the count of published events.
    #[must_use]
    pub fn get_event_count(&self) -> usize {
        self.event_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl PeerDiscoveryEventPublisher for NoOpEventPublisher {
    fn publish(&self, _event: PeerDiscoveryEventPayload) -> Result<(), String> {
        self.event_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn publish_response(
        &self,
        _topic: &str,
        _correlation_id: [u8; 16],
        _response: PeerListResponsePayload,
    ) -> Result<(), String> {
        self.event_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

/// In-memory publisher for testing that stores events.
#[derive(Debug, Default)]
pub struct InMemoryEventPublisher {
    events: std::sync::Mutex<Vec<PeerDiscoveryEventPayload>>,
}

impl InMemoryEventPublisher {
    /// Create a new in-memory publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all published events.
    #[must_use]
    pub fn get_events(&self) -> Vec<PeerDiscoveryEventPayload> {
        self.events.lock().unwrap().clone()
    }

    /// Clear all stored events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

impl PeerDiscoveryEventPublisher for InMemoryEventPublisher {
    fn publish(&self, event: PeerDiscoveryEventPayload) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }

    fn publish_response(
        &self,
        _topic: &str,
        _correlation_id: [u8; 16],
        response: PeerListResponsePayload,
    ) -> Result<(), String> {
        self.events
            .lock()
            .unwrap()
            .push(PeerDiscoveryEventPayload::PeerListResponse(response));
        Ok(())
    }
}

/// No-op verification request publisher for testing.
#[derive(Debug, Default)]
pub struct NoOpVerificationPublisher {
    /// Count of requests published.
    pub request_count: std::sync::atomic::AtomicUsize,
}

impl NoOpVerificationPublisher {
    /// Create a new no-op publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            request_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get the count of published requests.
    #[must_use]
    pub fn get_request_count(&self) -> usize {
        self.request_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

macro_rules! impl_verification_publisher {
    ($type:ty, $body:expr) => {
        impl VerificationRequestPublisher for $type {
            fn publish_verification_request(
                &self,
                request: VerifyNodeIdentityRequest,
                correlation_id: [u8; 16],
            ) -> Result<(), String> {
                $body(self, request, correlation_id)
            }
        }
    };
}

impl_verification_publisher!(
    NoOpVerificationPublisher,
    |this: &NoOpVerificationPublisher, _, _| {
        this.request_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
);

/// In-memory verification publisher for testing.
#[derive(Debug, Default)]
pub struct InMemoryVerificationPublisher {
    requests: std::sync::Mutex<Vec<(VerifyNodeIdentityRequest, [u8; 16])>>,
}

impl InMemoryVerificationPublisher {
    /// Create a new in-memory publisher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Get all published requests.
    #[must_use]
    pub fn get_requests(&self) -> Vec<(VerifyNodeIdentityRequest, [u8; 16])> {
        self.requests.lock().unwrap().clone()
    }

    /// Clear all stored requests.
    pub fn clear(&self) {
        self.requests.lock().unwrap().clear();
    }
}

impl_verification_publisher!(
    InMemoryVerificationPublisher,
    |this: &InMemoryVerificationPublisher, req, cid| {
        this.requests.lock().unwrap().push((req, cid));
        Ok(())
    }
);
