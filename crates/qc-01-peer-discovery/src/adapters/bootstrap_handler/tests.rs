use super::*;
use crate::adapters::{InMemoryVerificationPublisher, NoOpNodeIdValidator};
use crate::domain::{IpAddr, KademliaConfig, NodeId, Timestamp};
use crate::ipc::{BootstrapRequest, BootstrapResult};
use crate::ports::TimeSource;
use crate::service::PeerDiscoveryService;
// ProofOfWork is now re-exported by super (mod.rs)

/// Fixed time source for testing.
struct TestTimeSource(Timestamp);

impl TestTimeSource {
    fn new(ts: Timestamp) -> Self {
        Self(ts)
    }
}

impl TimeSource for TestTimeSource {
    fn now(&self) -> Timestamp {
        self.0
    }
}

fn make_handler() -> BootstrapHandler<PeerDiscoveryService, InMemoryVerificationPublisher> {
    let local_id = NodeId::new([0u8; 32]);
    let config = KademliaConfig::for_testing();
    let time_source: Box<dyn TimeSource> = Box::new(TestTimeSource::new(Timestamp::new(1000)));
    let service = PeerDiscoveryService::new(local_id, config, time_source);
    let test_time: Box<dyn TimeSource> = Box::new(TestTimeSource::new(Timestamp::new(1000)));
    let publisher = InMemoryVerificationPublisher::new();
    let validator = Box::new(NoOpNodeIdValidator::new());

    // Use low difficulty (8 bits) for fast tests
    let config = BootstrapHandlerConfig {
        pow_validator: validator,
        time_source: test_time,
        pow_difficulty: 8,
    };

    BootstrapHandler::with_config(service, publisher, config)
}

/// Generate a valid PoW nonce for a given node_id at difficulty 8.
fn generate_test_pow(node_id: &[u8; 32]) -> [u8; 32] {
    // Use NodeId struct
    let node_id = NodeId::new(*node_id);

    let mut nonce = [0u8; 32];
    for i in 0..100_000u32 {
        nonce[0..4].copy_from_slice(&i.to_le_bytes());
        if ProofOfWork::new(nonce).validate(&node_id, 8) {
            return nonce;
        }
    }
    panic!("Failed to generate valid PoW");
}

fn make_request(node_byte: u8) -> BootstrapRequest {
    let mut node_id = [0u8; 32];
    node_id[0] = node_byte;

    // Generate valid PoW for this node_id
    let pow = generate_test_pow(&node_id);

    BootstrapRequest::new(crate::ipc::BootstrapRequestConfig {
        node_id,
        ip_address: IpAddr::v4(192, 168, 1, node_byte),
        port: 8080,
        proof_of_work: pow,
        claimed_pubkey: [2u8; 33],
        signature: [3u8; 64],
    })
}

#[test]
fn test_handle_valid_bootstrap_request() {
    let mut handler = make_handler();
    let request = make_request(1);

    let result = handler.handle(&request);

    match result {
        BootstrapResult::PendingVerification { correlation_id } => {
            // Correlation ID should be non-zero
            assert_ne!(correlation_id, [0u8; 16]);
        }
        other => panic!("Expected PendingVerification, got {:?}", other),
    }
}

#[test]
fn test_handle_invalid_pow() {
    let mut handler = make_handler();
    let mut request = make_request(1);
    // Invalid PoW - no leading zeros
    request.proof_of_work = [255u8; 32];

    let result = handler.handle(&request);

    assert_eq!(result, BootstrapResult::InvalidProofOfWork);
}
