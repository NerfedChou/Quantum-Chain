//! # Block Storage Bus Adapter
//!
//! Connects Block Storage to the shared event bus, handling message routing
//! and the stateful assembler pattern.
//!
//! ## Note
//!
//! The actual message serialization/deserialization is stubbed out until
//! serde is integrated. The structure and event routing logic is in place.

use crate::ipc::handlers::HandlerError;
use crate::ipc::BlockStorageHandler;
use crate::ports::outbound::{
    BlockSerializer, ChecksumProvider, FileSystemAdapter, KeyValueStore, TimeSource,
};
use crate::service::BlockStorageService;

use super::event_types;

/// Error types for the bus adapter
#[derive(Debug)]
pub enum BusAdapterError {
    /// Failed to deserialize message from bus
    Deserialization(String),
    /// Failed to serialize message for bus
    Serialization(String),
    /// Handler returned an error
    Handler(HandlerError),
    /// Bus operation failed
    BusError(String),
    /// Unknown event type
    UnknownEventType(String),
}

impl std::fmt::Display for BusAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deserialization(msg) => write!(f, "Deserialization error: {}", msg),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::Handler(e) => write!(f, "Handler error: {:?}", e),
            Self::BusError(msg) => write!(f, "Bus error: {}", msg),
            Self::UnknownEventType(t) => write!(f, "Unknown event type: {}", t),
        }
    }
}

impl std::error::Error for BusAdapterError {}

/// Adapter that connects BlockStorageHandler to the shared event bus.
///
/// This adapter:
/// 1. Subscribes to incoming events (BlockValidated, MerkleRootComputed, StateRootComputed)
/// 2. Routes them to the appropriate handler method
/// 3. Publishes outgoing events (BlockStored, BlockFinalized)
///
/// ## Example
///
/// ```ignore
/// use qc_02_block_storage::bus::BlockStorageBusAdapter;
///
/// let adapter = BlockStorageBusAdapter::new(handler);
///
/// // In an async context:
/// adapter.handle_event(event_type, payload).await?;
/// ```
pub struct BlockStorageBusAdapter<KV, FS, CS, TS, SER>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    SER: BlockSerializer,
{
    handler: BlockStorageHandler<KV, FS, CS, TS, SER>,
    /// Callback for publishing events (set by runtime)
    publish_callback: Option<Box<dyn Fn(&str, Vec<u8>) -> Result<(), String> + Send + Sync>>,
}

impl<KV, FS, CS, TS, SER> BlockStorageBusAdapter<KV, FS, CS, TS, SER>
where
    KV: KeyValueStore,
    FS: FileSystemAdapter,
    CS: ChecksumProvider,
    TS: TimeSource,
    SER: BlockSerializer,
{
    /// Create a new bus adapter wrapping the given handler.
    pub fn new(handler: BlockStorageHandler<KV, FS, CS, TS, SER>) -> Self {
        Self {
            handler,
            publish_callback: None,
        }
    }

    /// Set the callback for publishing events to the bus.
    ///
    /// This is called by the runtime when wiring up the adapter.
    pub fn set_publish_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str, Vec<u8>) -> Result<(), String> + Send + Sync + 'static,
    {
        self.publish_callback = Some(Box::new(callback));
    }

    /// Get the list of event types this adapter subscribes to.
    pub fn subscriptions() -> Vec<&'static str> {
        vec![
            event_types::BLOCK_VALIDATED,
            event_types::MERKLE_ROOT_COMPUTED,
            event_types::STATE_ROOT_COMPUTED,
            event_types::MARK_FINALIZED,
        ]
    }

    /// Handle an incoming event from the bus.
    ///
    /// Routes the event to the appropriate handler method based on event type.
    /// If the handler produces an outgoing event, it is published via the callback.
    ///
    /// ## Note
    ///
    /// The actual deserialization is stubbed until serde is integrated.
    /// This method validates the event type routing logic.
    pub fn handle_event(
        &mut self,
        event_type: &str,
        _payload: &[u8],
    ) -> Result<(), BusAdapterError> {
        match event_type {
            event_types::BLOCK_VALIDATED => {
                // TODO: Deserialize and call self.handler.handle_block_validated(msg)
                // For now, just validate the routing
                Ok(())
            }

            event_types::MERKLE_ROOT_COMPUTED => {
                // TODO: Deserialize and call self.handler.handle_merkle_root_computed(msg)
                Ok(())
            }

            event_types::STATE_ROOT_COMPUTED => {
                // TODO: Deserialize and call self.handler.handle_state_root_computed(msg)
                Ok(())
            }

            event_types::MARK_FINALIZED => {
                // TODO: Deserialize and call self.handler.handle_mark_finalized(msg)
                Ok(())
            }

            unknown => Err(BusAdapterError::UnknownEventType(unknown.to_string())),
        }
    }

    /// Run garbage collection on expired assemblies.
    ///
    /// This should be called periodically (e.g., every 5 seconds) by the runtime.
    /// Returns the timeout payloads for monitoring.
    pub fn gc_expired_assemblies(&mut self) -> Vec<crate::ipc::payloads::AssemblyTimeoutPayload> {
        self.handler.gc_expired_assemblies()
    }

    /// Get the hashes of expired assemblies.
    ///
    /// Convenience method that extracts just the block hashes.
    pub fn gc_expired_assembly_hashes(&mut self) -> Vec<[u8; 32]> {
        let expired = self.handler.gc_expired_assemblies();

        // Publish timeout events for monitoring
        for timeout in &expired {
            // Best effort - don't fail GC if publish fails
            let _ = self.publish(event_types::ASSEMBLY_TIMEOUT, timeout.block_hash.to_vec());
        }

        expired.into_iter().map(|t| t.block_hash).collect()
    }

    /// Access the underlying service for queries.
    pub fn service(&self) -> &BlockStorageService<KV, FS, CS, TS, SER> {
        self.handler.service()
    }

    /// Publish an event to the bus.
    fn publish(&self, event_type: &str, data: Vec<u8>) -> Result<(), BusAdapterError> {
        if let Some(ref callback) = self.publish_callback {
            callback(event_type, data).map_err(BusAdapterError::BusError)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::outbound::{
        BincodeBlockSerializer, DefaultChecksumProvider, InMemoryKVStore, MockFileSystemAdapter,
        SystemTimeSource,
    };
    use crate::service::BlockStorageService;
    use crate::StorageConfig;

    fn make_test_adapter() -> BlockStorageBusAdapter<
        InMemoryKVStore,
        MockFileSystemAdapter,
        DefaultChecksumProvider,
        SystemTimeSource,
        BincodeBlockSerializer,
    > {
        let service = BlockStorageService::new(
            InMemoryKVStore::new(),
            MockFileSystemAdapter::new(50),
            DefaultChecksumProvider,
            SystemTimeSource,
            BincodeBlockSerializer,
            StorageConfig::default(),
        );
        let shared_secret = [0u8; 32];
        let handler = BlockStorageHandler::new(service, shared_secret);
        BlockStorageBusAdapter::new(handler)
    }

    #[test]
    fn test_subscriptions_returns_expected_events() {
        let subs = BlockStorageBusAdapter::<
            InMemoryKVStore,
            MockFileSystemAdapter,
            DefaultChecksumProvider,
            SystemTimeSource,
            BincodeBlockSerializer,
        >::subscriptions();

        assert!(subs.contains(&event_types::BLOCK_VALIDATED));
        assert!(subs.contains(&event_types::MERKLE_ROOT_COMPUTED));
        assert!(subs.contains(&event_types::STATE_ROOT_COMPUTED));
        assert!(subs.contains(&event_types::MARK_FINALIZED));
        assert_eq!(subs.len(), 4);
    }

    #[test]
    fn test_unknown_event_type_returns_error() {
        let mut adapter = make_test_adapter();
        let result = adapter.handle_event("UnknownEvent", &[]);

        assert!(matches!(result, Err(BusAdapterError::UnknownEventType(_))));
    }

    #[test]
    fn test_gc_expired_assemblies_runs_without_panic() {
        let mut adapter = make_test_adapter();
        let expired = adapter.gc_expired_assembly_hashes();

        // No assemblies added, so none expired
        assert!(expired.is_empty());
    }

    #[test]
    fn test_set_publish_callback() {
        let mut adapter = make_test_adapter();
        let published = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let published_clone = published.clone();

        adapter.set_publish_callback(move |event_type, data| {
            published_clone
                .lock()
                .unwrap()
                .push((event_type.to_string(), data));
            Ok(())
        });

        // Verify callback was set
        assert!(adapter.publish_callback.is_some());
    }
}
