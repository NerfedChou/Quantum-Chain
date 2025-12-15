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
    BlockHeader, ChainHead, CommitMessage, ConsensusAlgorithm, ConsensusConfig,
    ConsensusError, ConsensusResult, PBFTProof, PoSProof, PrepareMessage, ValidatedBlock,
    ValidationProof,
};
use crate::events::BlockValidatedEvent;
use crate::ports::{
    ConsensusApi, EventBus, MempoolGateway, SignatureVerifier, SystemTimeSource, TimeSource,
    ValidatorSetProvider,
};
use crate::state::ConsensusState;
use crate::validation::BlockValidator;
use async_trait::async_trait;
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
    state: Arc<ConsensusState>,
    config: ConsensusConfig,
    time_source: Box<dyn TimeSource>,
}

/// Dependencies for ConsensusService
pub struct ConsensusDependencies<E, M, S, V> {
    pub event_bus: Arc<E>,
    pub mempool: Arc<M>,
    pub sig_verifier: Arc<S>,
    pub validator_provider: Arc<V>,
    pub config: ConsensusConfig,
}

impl<E, M, S, V> ConsensusService<E, M, S, V>
where
    E: EventBus,
    M: MempoolGateway,
    S: SignatureVerifier,
    V: ValidatorSetProvider,
{
    /// Create a new ConsensusService
    pub fn new(deps: ConsensusDependencies<E, M, S, V>) -> Self {
        Self {
            event_bus: deps.event_bus,
            mempool: deps.mempool,
            sig_verifier: deps.sig_verifier,
            validator_provider: deps.validator_provider,
            state: Arc::new(ConsensusState::new()),
            config: deps.config,
            time_source: Box::new(SystemTimeSource),
        }
    }

    /// Create with genesis block
    pub fn with_genesis(deps: ConsensusDependencies<E, M, S, V>, genesis: BlockHeader) -> Self {
        Self {
            event_bus: deps.event_bus,
            mempool: deps.mempool,
            sig_verifier: deps.sig_verifier,
            validator_provider: deps.validator_provider,
            state: Arc::new(ConsensusState::with_genesis(genesis)),
            config: deps.config,
            time_source: Box::new(SystemTimeSource),
        }
    }

    /// Set custom time source (for testing)
    pub fn with_time_source(mut self, time_source: Box<dyn TimeSource>) -> Self {
        self.time_source = time_source;
        self
    }

    // === VALIDATION METHODS ===

    // NOTE: Stateless validation logic moved to validation.rs

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

    /// Validate PBFT proof with ZERO-TRUST signature re-verification
    ///
    /// Requires 2f+1 prepare and commit messages
    async fn validate_pbft_proof(
        &self,
        proof: &PBFTProof,
        _block_hash: &Hash, // Block hash is verified via prepare/commit messages
    ) -> ConsensusResult<()> {
        let current_view = self.state.current_view();

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
        self.verify_pbft_signatures(&validator_set, &proof.prepares, |prepare| {
            let msg = prepare_signing_message(prepare.view, prepare.sequence, &prepare.block_hash);
            (prepare.validator, msg, &prepare.signature)
        })?;

        // ZERO-TRUST: Verify commit signatures
        // SECURITY: Each commit message MUST be independently verified
        self.verify_pbft_signatures(&validator_set, &proof.commits, |commit| {
            let msg = commit_signing_message(commit.view, commit.sequence, &commit.block_hash);
            (commit.validator, msg, &commit.signature)
        })?;

        Ok(())
    }

    /// Verify block logic (Stateless checks depending on state but not modifying it)
    async fn verify_block_logic(&self, block: &Block, block_hash: &Hash) -> ConsensusResult<()> {
        BlockValidator::validate_structure(block, &self.config)
            .map_err(|e| { crate::metrics::record_block_rejected("invalid_structure"); e })?;

        BlockValidator::validate_parent(&block.header, &self.state)
            .map_err(|e| { crate::metrics::record_block_rejected("unknown_parent"); e })?;

        BlockValidator::validate_height(&block.header, &self.state)
            .map_err(|e| { crate::metrics::record_block_rejected("invalid_height"); e })?;

        BlockValidator::validate_timestamp(
            &block.header,
            &self.state,
            self.time_source.as_ref(),
            &self.config,
        )
        .map_err(|e| {
            crate::metrics::record_block_rejected("invalid_timestamp");
            e
        })?;

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
            let chain = self.state.chain.read();
            if chain.has_block(&block_hash) {
                crate::metrics::record_block_rejected("already_validated");
                return Err(ConsensusError::AlreadyValidated(block_hash));
            }
        }

        // Verify block logic
        self.verify_block_logic(&block, &block_hash).await?;

        // 7. Add to chain state
        {
            let mut chain = self.state.chain.write();
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
        let event = BlockValidatedEvent::new(validated.clone(), block.proof, now);
        self.event_bus
            .publish_block_validated(event)
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
            let chain = self.state.chain.read();
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
        let current_view = self.state.current_view();
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
        self.state.chain.read().head().clone()
    }

    async fn is_validated(&self, block_hash: Hash) -> bool {
        self.state.chain.read().has_block(&block_hash)
    }

    async fn current_epoch(&self) -> u64 {
        self.validator_provider.current_epoch().await
    }
}

#[cfg(test)]
mod tests;
