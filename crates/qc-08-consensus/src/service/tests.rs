use super::*;
use crate::domain::{Attestation, SignedTransaction, ValidatorInfo, ValidatorSet};
use std::sync::atomic::{AtomicU64, Ordering};

// Mock implementations for testing
struct MockEventBus {
    published_count: AtomicU64,
}

impl MockEventBus {
    fn new() -> Self {
        Self {
            published_count: AtomicU64::new(0),
        }
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish_block_validated(
        &self,
        _event: BlockValidatedEvent,
    ) -> Result<(), String> {
        self.published_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct MockMempool;

#[async_trait]
impl MempoolGateway for MockMempool {
    async fn get_transactions_for_block(
        &self,
        _max_count: usize,
        _max_gas: u64,
    ) -> Result<Vec<SignedTransaction>, String> {
        Ok(vec![])
    }

    async fn propose_transactions(
        &self,
        _tx_hashes: Vec<Hash>,
        _target_block_height: u64,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct MockSigVerifier;

impl SignatureVerifier for MockSigVerifier {
    fn verify_ecdsa(&self, _msg: &[u8], _sig: &[u8; 65], _pk: &[u8; 33]) -> bool {
        true
    }

    fn verify_aggregate_bls(&self, _msg: &[u8], _sig: &[u8; 96], _pks: &[[u8; 48]]) -> bool {
        true
    }

    fn recover_signer(&self, _msg: &[u8], _sig: &[u8; 65]) -> Option<[u8; 20]> {
        Some([0u8; 20])
    }
}

struct MockValidatorProvider {
    validators: Vec<ValidatorInfo>,
}

impl MockValidatorProvider {
    fn new(count: usize) -> Self {
        let validators = (0..count)
            .map(|i| {
                let mut id = [0u8; 32];
                id[0] = i as u8;
                ValidatorInfo::new(id, 100, [i as u8; 48])
            })
            .collect();
        Self { validators }
    }
}

#[async_trait]
impl ValidatorSetProvider for MockValidatorProvider {
    async fn get_validator_set_at_epoch(
        &self,
        epoch: u64,
        _state_root: Hash,
    ) -> Result<ValidatorSet, String> {
        Ok(ValidatorSet::new(epoch, self.validators.clone()))
    }

    async fn get_total_stake_at_epoch(
        &self,
        _epoch: u64,
        _state_root: Hash,
    ) -> Result<u128, String> {
        Ok(self.validators.iter().map(|v| v.stake).sum())
    }

    async fn current_epoch(&self) -> u64 {
        1
    }

    async fn get_epoch_state_root(&self, _epoch: u64) -> Result<Hash, String> {
        Ok([0u8; 32])
    }
}

fn create_test_deps(
    validator_count: usize,
) -> ConsensusDependencies<MockEventBus, MockMempool, MockSigVerifier, MockValidatorProvider> {
    ConsensusDependencies {
        event_bus: Arc::new(MockEventBus::new()),
        mempool: Arc::new(MockMempool),
        sig_verifier: Arc::new(MockSigVerifier),
        validator_provider: Arc::new(MockValidatorProvider::new(validator_count)),
        config: ConsensusConfig::default(),
    }
}

fn create_test_service(
    validator_count: usize,
) -> ConsensusService<MockEventBus, MockMempool, MockSigVerifier, MockValidatorProvider> {
    ConsensusService::new(create_test_deps(validator_count))
}

fn create_genesis() -> BlockHeader {
    BlockHeader {
        version: 1,
        block_height: 0,
        parent_hash: [0u8; 32],
        timestamp: 1000,
        proposer: [0u8; 32],
        transactions_root: None,
        state_root: None,
        receipts_root: [0u8; 32],
        gas_limit: 30_000_000,
        gas_used: 0,
        extra_data: vec![],
    }
}

fn create_valid_block(parent: &BlockHeader, attestation_count: usize) -> Block {
    // First validator is the proposer
    let mut proposer = [0u8; 32];
    proposer[0] = 0;

    let header = BlockHeader {
        version: 1,
        block_height: parent.block_height + 1,
        parent_hash: parent.hash(),
        timestamp: parent.timestamp + 12,
        proposer,
        transactions_root: None,
        state_root: None,
        receipts_root: [0u8; 32],
        gas_limit: 30_000_000,
        gas_used: 0,
        extra_data: vec![],
    };

    let block_hash = header.hash();
    let attestations: Vec<Attestation> = (0..attestation_count)
        .map(|i| {
            let mut validator = [0u8; 32];
            validator[0] = i as u8;
            Attestation {
                validator,
                block_hash,
                signature: vec![0u8; 65], // Vec<u8> now
                slot: 0,
            }
        })
        .collect();

    Block {
        header,
        transactions: vec![],
        proof: ValidationProof::PoS(PoSProof {
            attestations,
            epoch: 1,
            slot: 0,
        }),
    }
}

#[tokio::test]
async fn test_validate_block_success() {
    let genesis = create_genesis();
    let service = ConsensusService::with_genesis(create_test_deps(3), genesis.clone());

    let block = create_valid_block(&genesis, 2); // 2/3 attestations
    let result = service.validate_block(block, None).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_validate_block_insufficient_attestations() {
    let genesis = create_genesis();
    let service = ConsensusService::with_genesis(create_test_deps(3), genesis.clone());

    let block = create_valid_block(&genesis, 1); // Only 1/3 attestations
    let result = service.validate_block(block, None).await;

    assert!(matches!(
        result,
        Err(ConsensusError::InsufficientAttestations { .. })
    ));
}

#[tokio::test]
async fn test_validate_block_unknown_parent() {
    let service = create_test_service(3);

    // Block with non-existent parent
    let mut header = create_genesis();
    header.block_height = 1;
    header.parent_hash = [0xFF; 32];

    let block = Block {
        header,
        transactions: vec![],
        proof: ValidationProof::PoS(PoSProof {
            attestations: vec![],
            epoch: 1,
            slot: 0,
        }),
    };

    let result = service.validate_block(block, None).await;
    assert!(matches!(result, Err(ConsensusError::UnknownParent(_))));
}

#[tokio::test]
async fn test_validate_block_height_skip() {
    let genesis = create_genesis();
    let service = ConsensusService::with_genesis(create_test_deps(3), genesis.clone());

    // Block that skips heights
    let mut block = create_valid_block(&genesis, 2);
    block.header.block_height = 5; // Skip to height 5

    let result = service.validate_block(block, None).await;
    assert!(matches!(result, Err(ConsensusError::InvalidHeight { .. })));
}

// =========================================================================
// PHASE 1: CRITICAL PRODUCTION TESTS
// =========================================================================

/// Signature verifier that always fails - for testing rejection paths
struct FailingSigVerifier;

impl SignatureVerifier for FailingSigVerifier {
    fn verify_ecdsa(&self, _msg: &[u8], _sig: &[u8; 65], _pk: &[u8; 33]) -> bool {
        false // Always fail
    }

    fn verify_aggregate_bls(&self, _msg: &[u8], _sig: &[u8; 96], _pks: &[[u8; 48]]) -> bool {
        false // Always fail
    }

    fn recover_signer(&self, _msg: &[u8], _sig: &[u8; 65]) -> Option<[u8; 20]> {
        None // Always fail
    }
}

/// Time source that returns a fixed timestamp - for testing timestamp validation
struct FixedTimeSource {
    timestamp: u64,
}

impl FixedTimeSource {
    fn new(timestamp: u64) -> Self {
        Self { timestamp }
    }
}

impl TimeSource for FixedTimeSource {
    fn now(&self) -> u64 {
        self.timestamp
    }

    fn current_epoch(&self, genesis_time: u64, epoch_length_secs: u64) -> u64 {
        if self.timestamp < genesis_time {
            return 0;
        }
        (self.timestamp - genesis_time) / epoch_length_secs
    }
}

// Import PBFT types for tests
use crate::domain::{CommitMessage, PBFTProof, PrepareMessage};

/// Helper to create a valid PBFT block
fn create_pbft_block(parent: &BlockHeader, prepare_count: usize, commit_count: usize) -> Block {
    let mut proposer = [0u8; 32];
    proposer[0] = 0;

    let header = BlockHeader {
        version: 1,
        block_height: parent.block_height + 1,
        parent_hash: parent.hash(),
        timestamp: parent.timestamp + 12,
        proposer,
        transactions_root: None,
        state_root: None,
        receipts_root: [0u8; 32],
        gas_limit: 30_000_000,
        gas_used: 0,
        extra_data: vec![],
    };

    let block_hash = header.hash();

    // Create prepare messages
    let prepares: Vec<PrepareMessage> = (0..prepare_count)
        .map(|i| {
            let mut validator = [0u8; 32];
            validator[0] = i as u8;
            PrepareMessage {
                view: 0,
                sequence: 1,
                block_hash,
                validator,
                signature: [0u8; 65],
            }
        })
        .collect();

    // Create commit messages
    let commits: Vec<CommitMessage> = (0..commit_count)
        .map(|i| {
            let mut validator = [0u8; 32];
            validator[0] = i as u8;
            CommitMessage {
                view: 0,
                sequence: 1,
                block_hash,
                validator,
                signature: [0u8; 65],
            }
        })
        .collect();

    Block {
        header,
        transactions: vec![],
        proof: ValidationProof::PBFT(PBFTProof {
            prepares,
            commits,
            view: 0,
            epoch: 1,
            //epoch: 1, // Fixed syntax error
        }),
    }
}

// -------------------------------------------------------------------------
// TEST 1: PBFT with valid 2f+1 prepares and commits
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_pbft_proof_success() {
    let genesis = create_genesis();

    // 4 validators: f=1, need 2f+1=3 votes
    let mut deps = create_test_deps(4);
    deps.config.algorithm = ConsensusAlgorithm::PBFT;
    deps.config.byzantine_threshold = 1;
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    // Create block with 3 prepares and 3 commits (2f+1 = 3)
    let block = create_pbft_block(&genesis, 3, 3);
    let result = service.validate_block(block, None).await;

    assert!(
        result.is_ok(),
        "PBFT validation should succeed with 2f+1 votes"
    );
}

// -------------------------------------------------------------------------
// TEST 2: PBFT fails with insufficient prepares
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_pbft_proof_insufficient_prepares() {
    let genesis = create_genesis();

    // 4 validators: f=1, need 2f+1=3 votes
    let mut deps = create_test_deps(4);
    deps.config.algorithm = ConsensusAlgorithm::PBFT;
    deps.config.byzantine_threshold = 1;
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    // Only 2 prepares (need 3)
    let block = create_pbft_block(&genesis, 2, 3);
    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::InsufficientAttestations { .. })),
        "PBFT should fail with insufficient prepares"
    );
}

// -------------------------------------------------------------------------
// TEST 3: PBFT fails with insufficient commits
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_pbft_proof_insufficient_commits() {
    let genesis = create_genesis();

    let mut deps = create_test_deps(4);
    deps.config.algorithm = ConsensusAlgorithm::PBFT;
    deps.config.byzantine_threshold = 1;
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    // 3 prepares but only 2 commits (need 3)
    let block = create_pbft_block(&genesis, 3, 2);
    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::InsufficientAttestations { .. })),
        "PBFT should fail with insufficient commits"
    );
}

// -------------------------------------------------------------------------
// TEST 4: PoS signature verification failure
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_pos_signature_invalid() {
    let genesis = create_genesis();

    // Use FailingSigVerifier - all signatures will fail
    // Manual dependencies to support FailingSigVerifier
    let deps = ConsensusDependencies {
        event_bus: Arc::new(MockEventBus::new()),
        mempool: Arc::new(MockMempool),
        sig_verifier: Arc::new(FailingSigVerifier),
        validator_provider: Arc::new(MockValidatorProvider::new(3)),
        config: ConsensusConfig::default(),
    };
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    let block = create_valid_block(&genesis, 2);
    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::SignatureVerificationFailed(_))),
        "Should reject block when signature verification fails"
    );
}

// -------------------------------------------------------------------------
// TEST 5: PBFT signature verification failure
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_pbft_signature_invalid() {
    let genesis = create_genesis();

    // Use FailingSigVerifier
    let deps = ConsensusDependencies {
        event_bus: Arc::new(MockEventBus::new()),
        mempool: Arc::new(MockMempool),
        sig_verifier: Arc::new(FailingSigVerifier),
        validator_provider: Arc::new(MockValidatorProvider::new(4)),
        config: ConsensusConfig {
            algorithm: ConsensusAlgorithm::PBFT,
            byzantine_threshold: 1,
            ..ConsensusConfig::default()
        },
    };
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    let block = create_pbft_block(&genesis, 3, 3);
    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::SignatureVerificationFailed(_))),
        "PBFT should reject block when signature verification fails"
    );
}

// -------------------------------------------------------------------------
// TEST 6: Block with future timestamp rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_block_future_timestamp() {
    let genesis = create_genesis();

    // Create service with fixed time source at timestamp 2000
    let mut deps = create_test_deps(3);
    deps.config.max_timestamp_drift_secs = 60;
    let service = ConsensusService::with_genesis(deps, genesis.clone())
        .with_time_source(Box::new(FixedTimeSource::new(2000)));

    // Create block with timestamp far in the future (2000 + 120 > 2000 + 60)
    let mut block = create_valid_block(&genesis, 2);
    block.header.timestamp = 2200; // 200 seconds in future, drift is 60

    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::FutureTimestamp { .. })),
        "Should reject block with timestamp too far in the future"
    );
}

// -------------------------------------------------------------------------
// TEST 7: Duplicate attestations rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_block_duplicate_attestations() {
    let genesis = create_genesis();

    let service = ConsensusService::with_genesis(create_test_deps(3), genesis.clone());

    // Create block with duplicate attestation from same validator
    let mut block = create_valid_block(&genesis, 2);

    // Modify to have duplicate validator
    if let ValidationProof::PoS(ref mut proof) = block.proof {
        // Make both attestations from validator 0
        proof.attestations[1].validator = proof.attestations[0].validator;
    }

    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::DuplicateVote(_))),
        "Should reject block with duplicate attestations from same validator"
    );
}

// -------------------------------------------------------------------------
// TEST 8: Extra data too large rejected
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_block_extra_data_too_large() {
    let genesis = create_genesis();

    let service = ConsensusService::with_genesis(create_test_deps(3), genesis.clone());

    // Create block with oversized extra_data (limit is 32 bytes)
    let mut block = create_valid_block(&genesis, 2);
    block.header.extra_data = vec![0u8; 100]; // 100 bytes > 32 byte limit

    let result = service.validate_block(block, None).await;

    assert!(
        matches!(result, Err(ConsensusError::ExtraDataTooLarge { .. })),
        "Should reject block with extra_data exceeding limit"
    );
}

// -------------------------------------------------------------------------
// TEST 9: Event bus publish is called on success
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_validate_block_publishes_event() {
    let genesis = create_genesis();
    let event_bus = Arc::new(MockEventBus::new());

    let mut deps = create_test_deps(3);
    deps.event_bus = Arc::clone(&event_bus);
    let service = ConsensusService::with_genesis(deps, genesis.clone());

    let block = create_valid_block(&genesis, 2);
    let result = service.validate_block(block, None).await;

    assert!(result.is_ok());
    assert_eq!(
        event_bus.published_count.load(Ordering::SeqCst),
        1,
        "Should publish exactly one BlockValidated event"
    );
}
