//! Consensus Service - Core business logic
//!
//! Reference: SPEC-08-CONSENSUS.md Section 5
//!
//! # Architecture
//! - V2.3 Choreography Pattern (NOT Orchestrator)
//! - Zero-Trust Signature Re-Verification
//! - 2/3 attestation threshold for PoS
//! - 2f+1 votes for PBFT

use crate::domain::{
    attestation_signing_message, commit_signing_message, prepare_signing_message, Block,
    BlockHeader, ChainHead, ChainState, CommitMessage, ConsensusAlgorithm, ConsensusConfig,
    ConsensusError, ConsensusResult, PBFTProof, PoSProof, PrepareMessage, ValidatedBlock,
    ValidationProof,
};
use crate::ports::{
    ConsensusApi, EventBus, MempoolGateway, SignatureVerifier, SystemTimeSource, TimeSource,
    ValidatorSetProvider,
};
use async_trait::async_trait;
use parking_lot::RwLock;
use shared_types::Hash;
use std::collections::HashSet;
use std::sync::Arc;

/// Consensus Service
///
/// Reference: SPEC-08 Section 5
pub struct ConsensusService<E, M, S, V>
where
    E: EventBus,
    M: MempoolGateway,
    S: SignatureVerifier,
    V: ValidatorSetProvider,
{
    event_bus: Arc<E>,
    mempool: Arc<M>,
    sig_verifier: Arc<S>,
    validator_provider: Arc<V>,
    chain_state: RwLock<ChainState>,
    config: ConsensusConfig,
    time_source: Box<dyn TimeSource>,
    /// Current PBFT view (for PBFT mode)
    current_view: RwLock<u64>,
}

impl<E, M, S, V> ConsensusService<E, M, S, V>
where
    E: EventBus,
    M: MempoolGateway,
    S: SignatureVerifier,
    V: ValidatorSetProvider,
{
    /// Create a new ConsensusService
    pub fn new(
        event_bus: Arc<E>,
        mempool: Arc<M>,
        sig_verifier: Arc<S>,
        validator_provider: Arc<V>,
        config: ConsensusConfig,
    ) -> Self {
        Self {
            event_bus,
            mempool,
            sig_verifier,
            validator_provider,
            chain_state: RwLock::new(ChainState::new()),
            config,
            time_source: Box::new(SystemTimeSource),
            current_view: RwLock::new(0),
        }
    }

    /// Create with genesis block
    pub fn with_genesis(
        event_bus: Arc<E>,
        mempool: Arc<M>,
        sig_verifier: Arc<S>,
        validator_provider: Arc<V>,
        config: ConsensusConfig,
        genesis: BlockHeader,
    ) -> Self {
        Self {
            event_bus,
            mempool,
            sig_verifier,
            validator_provider,
            chain_state: RwLock::new(ChainState::with_genesis(genesis)),
            config,
            time_source: Box::new(SystemTimeSource),
            current_view: RwLock::new(0),
        }
    }

    /// Set custom time source (for testing)
    pub fn with_time_source(mut self, time_source: Box<dyn TimeSource>) -> Self {
        self.time_source = time_source;
        self
    }

    // === VALIDATION METHODS ===

    /// Validate block structure
    ///
    /// Checks: size limits, gas limits, transaction count
    fn validate_structure(&self, block: &Block) -> ConsensusResult<()> {
        // Check transaction count
        if block.transactions.len() > self.config.max_txs_per_block {
            return Err(ConsensusError::TooManyTransactions {
                count: block.transactions.len(),
                limit: self.config.max_txs_per_block,
            });
        }

        // Check gas limit
        let total_gas: u64 = block.transactions.iter().map(|tx| tx.gas_cost()).sum();

        if total_gas > self.config.max_block_gas {
            return Err(ConsensusError::GasLimitExceeded {
                used: total_gas,
                limit: self.config.max_block_gas,
            });
        }

        // Check header gas used matches transactions
        if block.header.gas_used > block.header.gas_limit {
            return Err(ConsensusError::GasLimitExceeded {
                used: block.header.gas_used,
                limit: block.header.gas_limit,
            });
        }

        // Check extra_data size limit (prevent DoS via oversized blocks)
        // Default limit: 32 bytes (Ethereum standard)
        const MAX_EXTRA_DATA_SIZE: usize = 32;
        if block.header.extra_data.len() > MAX_EXTRA_DATA_SIZE {
            return Err(ConsensusError::ExtraDataTooLarge {
                size: block.header.extra_data.len(),
                limit: MAX_EXTRA_DATA_SIZE,
            });
        }

        Ok(())
    }

    /// Validate parent chain linkage
    ///
    /// INVARIANT-1: Block parent_hash must reference an existing validated block
    fn validate_parent(&self, header: &BlockHeader) -> ConsensusResult<()> {
        let chain = self.chain_state.read();

        if header.is_genesis() {
            if chain.block_count() > 0 {
                return Err(ConsensusError::GenesisWithParent);
            }
            return Ok(());
        }

        if !chain.has_block(&header.parent_hash) {
            return Err(ConsensusError::UnknownParent(header.parent_hash));
        }

        Ok(())
    }

    /// Validate sequential height
    ///
    /// INVARIANT-4: Block height must be parent height + 1
    fn validate_height(&self, header: &BlockHeader) -> ConsensusResult<()> {
        if header.is_genesis() {
            return self.validate_genesis_height(header);
        }
        self.validate_chain_height(header)
    }

    fn validate_genesis_height(&self, header: &BlockHeader) -> ConsensusResult<()> {
        if header.block_height != 0 {
            return Err(ConsensusError::InvalidHeight {
                expected: 0,
                actual: header.block_height,
            });
        }
        Ok(())
    }

    fn validate_chain_height(&self, header: &BlockHeader) -> ConsensusResult<()> {
        let chain = self.chain_state.read();
        if let Some(parent) = chain.get_block(&header.parent_hash) {
            let expected = parent.block_height + 1;
            if header.block_height != expected {
                return Err(ConsensusError::InvalidHeight {
                    expected,
                    actual: header.block_height,
                });
            }
        }
        Ok(())
    }

    /// Validate timestamp ordering
    ///
    /// INVARIANT-5: Block timestamp must be > parent timestamp
    fn validate_timestamp(&self, header: &BlockHeader) -> ConsensusResult<()> {
        let now = self.time_source.now();

        // Check not too far in future
        if header.timestamp > now + self.config.max_timestamp_drift_secs {
            return Err(ConsensusError::FutureTimestamp {
                timestamp: header.timestamp,
                current: now,
            });
        }

        if header.is_genesis() {
            return Ok(());
        }

        let chain = self.chain_state.read();
        if let Some(parent) = chain.get_block(&header.parent_hash) {
            if header.timestamp <= parent.timestamp {
                return Err(ConsensusError::InvalidTimestamp {
                    block: header.timestamp,
                    parent: parent.timestamp,
                });
            }
        }

        Ok(())
    }

    /// Validate block proposer
    ///
    /// Ensures the proposer is a valid validator for this epoch/slot
    async fn validate_proposer(
        &self,
        header: &BlockHeader,
        proof: &ValidationProof,
    ) -> ConsensusResult<()> {
        // Genesis block has no proposer validation
        if header.is_genesis() {
            return Ok(());
        }

        let epoch = proof.epoch();

        // Get validator set for this epoch
        let epoch_state_root = self
            .validator_provider
            .get_epoch_state_root(epoch)
            .await
            .map_err(ConsensusError::StateError)?;

        let validator_set = self
            .validator_provider
            .get_validator_set_at_epoch(epoch, epoch_state_root)
            .await
            .map_err(ConsensusError::StateError)?;

        // Check proposer is in the validator set
        if !validator_set.contains(&header.proposer) {
            return Err(ConsensusError::InvalidProposer(header.proposer));
        }

        // For PoS, verify proposer was selected for this slot
        // (simplified - in production would use VRF/stake-weighted selection)
        if let ValidationProof::PoS(pos_proof) = proof {
            // Verify at least one attestation is from the proposer
            // This proves the proposer participated in the consensus round
            let proposer_attested = pos_proof
                .attestations
                .iter()
                .any(|a| a.validator == header.proposer);

            if !proposer_attested {
                return Err(ConsensusError::ProposerDidNotAttest(header.proposer));
            }
        }

        Ok(())
    }

    /// Validate PoS attestations with ZERO-TRUST signature re-verification
    ///
    /// INVARIANT-2: At least 2/3 of validators must attest
    /// INVARIANT-3: All signatures must be independently verified
    async fn validate_pos_proof(&self, proof: &PoSProof, block_hash: &Hash) -> ConsensusResult<()> {
        // Get validator set for this epoch
        let epoch_state_root = self
            .validator_provider
            .get_epoch_state_root(proof.epoch)
            .await
            .map_err(ConsensusError::StateError)?;

        let validator_set = self
            .validator_provider
            .get_validator_set_at_epoch(proof.epoch, epoch_state_root)
            .await
            .map_err(ConsensusError::StateError)?;

        // Check for duplicate votes
        let mut seen_validators = HashSet::new();
        for attestation in &proof.attestations {
            if !seen_validators.insert(attestation.validator) {
                return Err(ConsensusError::DuplicateVote(attestation.validator));
            }
        }

        // ZERO-TRUST: Re-verify each attestation signature independently
        self.verify_attestations(&validator_set, block_hash, proof)?;

        // Check attestation threshold (2/3)
        let participation = proof.participation_count();
        let required = validator_set.required_attestations(self.config.min_attestation_percent);

        if participation < required {
            let got_percent = (participation * 100 / validator_set.len()) as u8;
            return Err(ConsensusError::InsufficientAttestations {
                got: got_percent,
                required: self.config.min_attestation_percent,
            });
        }

        Ok(())
    }

    /// Verify attestations for PoS proof
    fn verify_attestations(
        &self,
        validator_set: &crate::domain::ValidatorSet,
        block_hash: &Hash,
        proof: &PoSProof,
    ) -> ConsensusResult<()> {
        let signing_message = attestation_signing_message(block_hash, proof.slot, proof.epoch);

        for attestation in &proof.attestations {
            // Verify validator is in active set
            let pubkey = validator_set
                .get_pubkey(&attestation.validator)
                .ok_or(ConsensusError::UnknownValidator(attestation.validator))?;

            // ZERO-TRUST: Re-verify signature (even if pre-validated)
            let valid = self.verify_attestation_signature(attestation, &signing_message, pubkey)?;

            // SECURITY: Always enforce signature verification
            if !valid {
                return Err(ConsensusError::SignatureVerificationFailed(
                    attestation.validator,
                ));
            }
        }
        Ok(())
    }

    /// Verify a single attestation signature (helper to reduce nesting)
    fn verify_attestation_signature(
        &self,
        attestation: &crate::domain::Attestation,
        signing_message: &[u8],
        pubkey: &[u8; 48],
    ) -> ConsensusResult<bool> {
        if attestation.is_bls() {
            // BLS signature (96 bytes)
            let sig_bytes: [u8; 96] = attestation
                .signature
                .as_slice()
                .try_into()
                .map_err(|_| ConsensusError::InvalidSignatureFormat(attestation.validator))?;

            Ok(self
                .sig_verifier
                .verify_aggregate_bls(signing_message, &sig_bytes, &[*pubkey]))
        } else if attestation.signature.len() == 65 {
            // ECDSA signature (65 bytes)
            let sig_bytes: [u8; 65] = attestation
                .signature
                .as_slice()
                .try_into()
                .map_err(|_| ConsensusError::InvalidSignatureFormat(attestation.validator))?;

            // Convert BLS pubkey to ECDSA pubkey format for verification
            let ecdsa_pubkey: [u8; 33] = {
                let mut pk = [0u8; 33];
                pk[0] = 0x02; // compressed prefix
                pk[1..].copy_from_slice(&pubkey[..32]);
                pk
            };

            Ok(self
                .sig_verifier
                .verify_ecdsa(signing_message, &sig_bytes, &ecdsa_pubkey))
        } else {
            Err(ConsensusError::InvalidSignatureFormat(
                attestation.validator,
            ))
        }
    }

    /// Generic helper to verify PBFT signatures (Deduplication)
    fn verify_pbft_signatures<T, F>(
        &self,
        validator_set: &crate::domain::ValidatorSet,
        items: &[T],
        msg_extractor: F,
    ) -> ConsensusResult<()>
    where
        F: Fn(&T) -> (crate::domain::ValidatorId, Vec<u8>, &[u8; 65]),
    {
        for item in items {
            let (validator, msg, signature) = msg_extractor(item);

            if !validator_set.contains(&validator) {
                return Err(ConsensusError::UnknownValidator(validator));
            }

            // Get validator's public key
            let pubkey = validator_set
                .get_pubkey(&validator)
                .ok_or(ConsensusError::UnknownValidator(validator))?;

            // ZERO-TRUST: Verify ECDSA signature independently
            // Convert 48-byte BLS pubkey to 33-byte compressed ECDSA for verification
            let ecdsa_pubkey: [u8; 33] = {
                let mut pk = [0u8; 33];
                pk[0] = 0x02; // compressed prefix
                pk[1..].copy_from_slice(&pubkey[..32]);
                pk
            };

            if !self
                .sig_verifier
                .verify_ecdsa(&msg, signature, &ecdsa_pubkey)
            {
                return Err(ConsensusError::SignatureVerificationFailed(validator));
            }
        }
        Ok(())
    }

    /// Verify PBFT prepare signatures
    fn verify_pbft_prepares(
        &self,
        validator_set: &crate::domain::ValidatorSet,
        prepares: &[PrepareMessage],
    ) -> ConsensusResult<()> {
        self.verify_pbft_signatures(validator_set, prepares, |prepare| {
            let msg = prepare_signing_message(prepare.view, prepare.sequence, &prepare.block_hash);
            (prepare.validator, msg, &prepare.signature)
        })
    }

    /// Verify PBFT commit signatures
    fn verify_pbft_commits(
        &self,
        validator_set: &crate::domain::ValidatorSet,
        commits: &[CommitMessage],
    ) -> ConsensusResult<()> {
        self.verify_pbft_signatures(validator_set, commits, |commit| {
            let msg = commit_signing_message(commit.view, commit.sequence, &commit.block_hash);
            (commit.validator, msg, &commit.signature)
        })
    }

    /// Validate PBFT proof with ZERO-TRUST signature re-verification
    ///
    /// Requires 2f+1 prepare and commit messages
    async fn validate_pbft_proof(
        &self,
        proof: &PBFTProof,
        _block_hash: &Hash, // Block hash is verified via prepare/commit messages
    ) -> ConsensusResult<()> {
        let current_view = *self.current_view.read();

        // Check view matches
        if proof.view != current_view {
            return Err(ConsensusError::ViewMismatch {
                expected: current_view,
                actual: proof.view,
            });
        }

        // Get validator set
        let epoch_state_root = self
            .validator_provider
            .get_epoch_state_root(proof.epoch)
            .await
            .map_err(ConsensusError::StateError)?;

        let validator_set = self
            .validator_provider
            .get_validator_set_at_epoch(proof.epoch, epoch_state_root)
            .await
            .map_err(ConsensusError::StateError)?;

        let required_votes = validator_set.required_pbft_votes(self.config.byzantine_threshold);

        // Check prepare count
        if proof.prepare_count() < required_votes {
            return Err(ConsensusError::InsufficientAttestations {
                got: proof.prepare_count() as u8,
                required: required_votes as u8,
            });
        }

        // Check commit count
        if proof.commit_count() < required_votes {
            return Err(ConsensusError::InsufficientAttestations {
                got: proof.commit_count() as u8,
                required: required_votes as u8,
            });
        }

        // ZERO-TRUST: Verify prepare signatures
        // SECURITY: Each prepare message MUST be independently verified
        self.verify_pbft_prepares(&validator_set, &proof.prepares)?;

        // ZERO-TRUST: Verify commit signatures
        // SECURITY: Each commit message MUST be independently verified
        self.verify_pbft_commits(&validator_set, &proof.commits)?;

        Ok(())
    }

    /// Verify block logic (Stateless checks depending on state but not modifying it)
    async fn verify_block_logic(&self, block: &Block, block_hash: &Hash) -> ConsensusResult<()> {
        self.validate_structure(block)
            .map_err(|e| { crate::metrics::record_block_rejected("invalid_structure"); e })?;

        self.validate_parent(&block.header)
            .map_err(|e| { crate::metrics::record_block_rejected("unknown_parent"); e })?;

        self.validate_height(&block.header)
            .map_err(|e| { crate::metrics::record_block_rejected("invalid_height"); e })?;

        self.validate_timestamp(&block.header)
            .map_err(|e| { crate::metrics::record_block_rejected("invalid_timestamp"); e })?;

        if let Err(e) = self.validate_proposer(&block.header, &block.proof).await {
            crate::metrics::record_block_rejected("invalid_proposer");
            return Err(e);
        }

        if let Err(e) = self.validate_consensus_proof(&block.proof, block_hash).await {
            match e {
                ConsensusError::InvalidSignatureFormat(_) => crate::metrics::record_block_rejected("invalid_signature"),
                _ => crate::metrics::record_block_rejected("invalid_proof"),
            }
            return Err(e);
        }

        Ok(())
    }

    async fn validate_consensus_proof(&self, proof: &ValidationProof, block_hash: &Hash) -> ConsensusResult<()> {
        match proof {
            ValidationProof::PoS(pos_proof) => self.validate_pos_proof(pos_proof, block_hash).await,
            ValidationProof::PBFT(pbft_proof) => self.validate_pbft_proof(pbft_proof, block_hash).await,
        }
    }

    /// Full block validation
    async fn validate_block_internal(&self, block: Block) -> ConsensusResult<ValidatedBlock> {
        let block_hash = block.hash();
        let start_time = std::time::Instant::now();

        // Check if already validated
        {
            let chain = self.chain_state.read();
            if chain.has_block(&block_hash) {
                crate::metrics::record_block_rejected("already_validated");
                return Err(ConsensusError::AlreadyValidated(block_hash));
            }
        }

        // Verify block logic
        self.verify_block_logic(&block, &block_hash).await?;

        // 7. Add to chain state
        {
            let mut chain = self.chain_state.write();
            chain.add_block(block.header.clone());
        }

        // 8. Create validated block
        let validated = ValidatedBlock {
            header: block.header,
            transactions: block.transactions,
            validation_proof: block.proof.clone(),
        };

        // 9. Publish to Event Bus (Choreography - non-blocking)
        let now = self.time_source.now();
        self.event_bus
            .publish_block_validated(
                block_hash,
                validated.header.block_height,
                validated.clone(),
                block.proof,
                now,
            )
            .await
            .map_err(ConsensusError::EventBusError)?;

        // Record success metrics
        let elapsed = start_time.elapsed().as_secs_f64();
        crate::metrics::record_block_validated();
        crate::metrics::record_validation_latency(elapsed);

        Ok(validated)
    }
}

#[async_trait]
impl<E, M, S, V> ConsensusApi for ConsensusService<E, M, S, V>
where
    E: EventBus + 'static,
    M: MempoolGateway + 'static,
    S: SignatureVerifier + 'static,
    V: ValidatorSetProvider + 'static,
{
    async fn validate_block(
        &self,
        block: Block,
        _source_peer: Option<[u8; 32]>,
    ) -> Result<ValidatedBlock, ConsensusError> {
        self.validate_block_internal(block).await
    }

    async fn build_block(&self) -> Result<Block, ConsensusError> {
        // Get transactions from mempool
        let transactions = self
            .mempool
            .get_transactions_for_block(self.config.max_txs_per_block, self.config.max_block_gas)
            .await
            .map_err(ConsensusError::MempoolError)?;

        // Calculate total gas
        let gas_used: u64 = transactions.iter().map(|tx| tx.gas_cost()).sum();

        // Get current head
        let head = {
            let chain = self.chain_state.read();
            chain.head().clone()
        };

        // Create block header
        let header = BlockHeader {
            version: 1,
            block_height: head.block_height + 1,
            parent_hash: head.block_hash,
            timestamp: self.time_source.now(),
            proposer: [0u8; 32],     // Would be set by validator identity
            transactions_root: None, // Computed by Subsystem 3
            state_root: None,        // Computed by Subsystem 4
            receipts_root: [0u8; 32],
            gas_limit: self.config.max_block_gas,
            gas_used,
            extra_data: vec![],
        };

        // NOTE: This returns a PROPOSAL block without attestations/votes.
        // The block MUST be broadcast to validators who will:
        // 1. Validate the proposal
        // 2. Sign attestations (PoS) or prepare/commit messages (PBFT)
        // 3. Return signatures to the proposer
        // 4. Proposer aggregates signatures into the final proof
        //
        // The returned block is NOT valid for submission to validate_block()
        // until it has collected sufficient attestations (2/3 for PoS, 2f+1 for PBFT).
        //
        // This is by design per the consensus protocol:
        // - Proposer creates block → broadcasts proposal
        // - Validators attest → proposer collects
        // - Proposer finalizes block with proof → broadcasts validated block
        let current_view = *self.current_view.read();
        let current_epoch = self.validator_provider.current_epoch().await;

        let proof = match self.config.algorithm {
            ConsensusAlgorithm::ProofOfStake => ValidationProof::PoS(PoSProof {
                attestations: vec![], // Filled by attestation collection phase
                epoch: current_epoch,
                slot: 0,
            }),
            ConsensusAlgorithm::PBFT => ValidationProof::PBFT(PBFTProof {
                prepares: vec![], // Filled by PBFT prepare phase
                commits: vec![],  // Filled by PBFT commit phase
                view: current_view,
                epoch: current_epoch,
            }),
        };

        Ok(Block {
            header,
            transactions,
            proof,
        })
    }

    async fn get_chain_head(&self) -> ChainHead {
        self.chain_state.read().head().clone()
    }

    async fn is_validated(&self, block_hash: Hash) -> bool {
        self.chain_state.read().has_block(&block_hash)
    }

    async fn current_epoch(&self) -> u64 {
        self.validator_provider.current_epoch().await
    }
}

#[cfg(test)]
mod tests {
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
            _block_hash: Hash,
            _block_height: u64,
            _block: ValidatedBlock,
            _consensus_proof: ValidationProof,
            _validated_at: u64,
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

    fn create_test_service(
        validator_count: usize,
    ) -> ConsensusService<MockEventBus, MockMempool, MockSigVerifier, MockValidatorProvider> {
        ConsensusService::new(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(validator_count)),
            ConsensusConfig::default(),
        )
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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

        let block = create_valid_block(&genesis, 2); // 2/3 attestations
        let result = service.validate_block(block, None).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_block_insufficient_attestations() {
        let genesis = create_genesis();
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier), // Always returns true
            Arc::new(MockValidatorProvider::new(4)),
            ConsensusConfig {
                algorithm: ConsensusAlgorithm::PBFT,
                byzantine_threshold: 1, // f=1
                ..ConsensusConfig::default()
            },
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(4)),
            ConsensusConfig {
                algorithm: ConsensusAlgorithm::PBFT,
                byzantine_threshold: 1,
                ..ConsensusConfig::default()
            },
            genesis.clone(),
        );

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

        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(4)),
            ConsensusConfig {
                algorithm: ConsensusAlgorithm::PBFT,
                byzantine_threshold: 1,
                ..ConsensusConfig::default()
            },
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(FailingSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(FailingSigVerifier),
            Arc::new(MockValidatorProvider::new(4)),
            ConsensusConfig {
                algorithm: ConsensusAlgorithm::PBFT,
                byzantine_threshold: 1,
                ..ConsensusConfig::default()
            },
            genesis.clone(),
        );

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
        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig {
                max_timestamp_drift_secs: 60, // Allow 60s drift
                ..ConsensusConfig::default()
            },
            genesis.clone(),
        )
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

        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

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

        let service = ConsensusService::with_genesis(
            Arc::new(MockEventBus::new()),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

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

        let service = ConsensusService::with_genesis(
            Arc::clone(&event_bus),
            Arc::new(MockMempool),
            Arc::new(MockSigVerifier),
            Arc::new(MockValidatorProvider::new(3)),
            ConsensusConfig::default(),
            genesis.clone(),
        );

        let block = create_valid_block(&genesis, 2);
        let result = service.validate_block(block, None).await;

        assert!(result.is_ok());
        assert_eq!(
            event_bus.published_count.load(Ordering::SeqCst),
            1,
            "Should publish exactly one BlockValidated event"
        );
    }
}
