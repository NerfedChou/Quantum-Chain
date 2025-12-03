//! Finality Service - Core business logic
//!
//! Reference: SPEC-09-FINALITY.md Section 5

use crate::domain::proof::FinalityProof;
use crate::domain::{
    AggregatedAttestations, Attestation, BlsSignature, Checkpoint, CheckpointId, CircuitBreaker,
    FinalityEvent, FinalityState, ValidatorId, ValidatorSet,
};
use crate::error::{FinalityError, FinalityResult};
use crate::ports::inbound::{AttestationResult, FinalityApi};
use crate::ports::outbound::{
    AttestationVerifier, BlockStorageGateway, MarkFinalizedRequest, ValidatorSetProvider,
};
use async_trait::async_trait;
use bitvec::prelude::*;
use parking_lot::RwLock;
use shared_types::Hash;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Slashable offense detected during attestation processing
#[derive(Clone, Debug)]
pub struct SlashableOffense {
    pub validator_id: ValidatorId,
    pub offense_type: SlashableOffenseType,
    pub attestation1: Attestation,
    pub attestation2: Attestation,
    pub detected_epoch: u64,
}

/// Type of slashable offense
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashableOffenseType {
    /// Same target epoch, different target block
    DoubleVote,
    /// One attestation surrounds another
    SurroundVote,
}

/// Finality configuration
#[derive(Clone, Debug)]
pub struct FinalityConfig {
    /// Blocks per epoch (checkpoint interval)
    pub epoch_length: u64,
    /// Required attestation percentage for justification
    pub justification_threshold_percent: u8,
    /// Maximum sync attempts before halt
    pub max_sync_attempts: u8,
    /// Sync attempt timeout (seconds)
    pub sync_timeout_secs: u64,
    /// Inactivity leak start (epochs without finality)
    pub inactivity_leak_epochs: u64,
    /// Always re-verify signatures (zero-trust)
    pub always_reverify_signatures: bool,
}

impl Default for FinalityConfig {
    fn default() -> Self {
        Self {
            epoch_length: 32,
            justification_threshold_percent: 67,
            max_sync_attempts: 3,
            sync_timeout_secs: 60,
            inactivity_leak_epochs: 4,
            always_reverify_signatures: true,
        }
    }
}

/// Internal state for finality tracking
struct FinalityServiceState {
    /// Circuit breaker for livelock prevention
    circuit_breaker: CircuitBreaker,
    /// Checkpoints by epoch
    checkpoints: HashMap<u64, Checkpoint>,
    /// Aggregated attestations by checkpoint
    attestations: HashMap<CheckpointId, AggregatedAttestations>,
    /// Finalized block hashes
    finalized_blocks: HashMap<Hash, u64>,
    /// Last finalized checkpoint
    last_finalized: Option<Checkpoint>,
    /// Last justified checkpoint
    last_justified: Option<Checkpoint>,
    /// Current epoch
    current_epoch: u64,
    /// Current head height
    current_height: u64,
    /// Epochs since last finality (for inactivity leak)
    epochs_without_finality: u64,
    /// Attestation history for slashing detection (validator_id -> attestations)
    attestation_history: HashMap<[u8; 32], Vec<Attestation>>,
    /// Detected slashable offenses
    slashable_offenses: Vec<SlashableOffense>,
}

impl FinalityServiceState {
    fn new() -> Self {
        Self {
            circuit_breaker: CircuitBreaker::new(),
            checkpoints: HashMap::new(),
            attestations: HashMap::new(),
            finalized_blocks: HashMap::new(),
            last_finalized: None,
            last_justified: None,
            current_epoch: 0,
            current_height: 0,
            epochs_without_finality: 0,
            attestation_history: HashMap::new(),
            slashable_offenses: Vec::new(),
        }
    }

    /// Check if inactivity leak should be triggered
    /// Reference: SPEC-09-FINALITY.md - inactivity_leak_epochs
    fn is_inactivity_leak_active(&self, config: &FinalityConfig) -> bool {
        self.epochs_without_finality >= config.inactivity_leak_epochs
    }
}

/// Finality Service implementation
///
/// Reference: SPEC-09-FINALITY.md Section 5
pub struct FinalityService<B, V, S>
where
    B: BlockStorageGateway,
    V: AttestationVerifier,
    S: ValidatorSetProvider,
{
    config: FinalityConfig,
    state: Arc<RwLock<FinalityServiceState>>,
    block_storage: Arc<B>,
    verifier: Arc<V>,
    validator_provider: Arc<S>,
}

impl<B, V, S> FinalityService<B, V, S>
where
    B: BlockStorageGateway,
    V: AttestationVerifier,
    S: ValidatorSetProvider,
{
    /// Create new finality service
    pub fn new(
        config: FinalityConfig,
        block_storage: Arc<B>,
        verifier: Arc<V>,
        validator_provider: Arc<S>,
    ) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(FinalityServiceState::new())),
            block_storage,
            verifier,
            validator_provider,
        }
    }

    /// Process a single attestation with zero-trust verification
    ///
    /// Reference: SPEC-09-FINALITY.md Appendix B.2 - Zero-Trust
    async fn process_single_attestation(
        &self,
        attestation: &Attestation,
        validators: &ValidatorSet,
    ) -> FinalityResult<Option<u128>> {
        // 1. Verify validator is in active set
        let validator_id = &attestation.validator_id;
        if !validators.contains(validator_id) {
            return Err(FinalityError::UnknownValidator {
                validator_id: validator_id.0,
            });
        }

        // 2. Zero-trust: Re-verify signature
        if self.config.always_reverify_signatures {
            if !self.verifier.verify_attestation(attestation) {
                return Err(FinalityError::InvalidSignature {
                    validator_id: validator_id.0,
                });
            }
        }

        // 3. Check for slashable conditions (double vote, surround vote)
        self.check_slashable_conditions(&attestation, &validator_id)?;

        // 4. Get stake weight
        let stake =
            validators
                .get_stake(validator_id)
                .ok_or_else(|| FinalityError::UnknownValidator {
                    validator_id: validator_id.0,
                })?;

        Ok(Some(stake))
    }

    /// Record attestation in history for slashing detection
    fn record_attestation(&self, state: &mut FinalityServiceState, attestation: &Attestation) {
        let history = state
            .attestation_history
            .entry(attestation.validator_id.0)
            .or_insert_with(Vec::new);

        // Keep only recent attestations (last 2 epochs worth)
        const MAX_HISTORY: usize = 64;
        if history.len() >= MAX_HISTORY {
            history.remove(0);
        }
        history.push(attestation.clone());
    }

    /// Check for slashable conditions (extracted to reduce nesting)
    fn check_slashable_conditions(
        &self,
        attestation: &Attestation,
        validator_id: &ValidatorId,
    ) -> Result<(), FinalityError> {
        let state = self.state.read();
        if let Some(history) = state.attestation_history.get(&validator_id.0) {
            for prev_att in history {
                if attestation.conflicts_with(prev_att) {
                    return Err(FinalityError::ConflictingAttestation);
                }
            }
        }
        Ok(())
    }

    /// Detect and record slashable offense
    #[allow(dead_code)] // Will be used when slashing is fully implemented
    fn detect_slashable_offense(
        &self,
        state: &mut FinalityServiceState,
        attestation: &Attestation,
        conflicting: &Attestation,
        current_epoch: u64,
    ) {
        let offense_type =
            if attestation.target_checkpoint.epoch == conflicting.target_checkpoint.epoch {
                SlashableOffenseType::DoubleVote
            } else {
                SlashableOffenseType::SurroundVote
            };

        let offense = SlashableOffense {
            validator_id: attestation.validator_id.clone(),
            offense_type,
            attestation1: attestation.clone(),
            attestation2: conflicting.clone(),
            detected_epoch: current_epoch,
        };

        tracing::warn!(
            "SLASHABLE OFFENSE DETECTED: {:?} by validator {:?}",
            offense_type,
            attestation.validator_id.0
        );

        state.slashable_offenses.push(offense);
    }

    /// Check if finalization is possible (two consecutive justified)
    ///
    /// INVARIANT-1: Finalization requires two consecutive justified checkpoints
    fn check_finalization(&self, state: &mut FinalityServiceState) -> Option<Checkpoint> {
        let last_justified = state.last_justified.as_ref()?;

        // Need previous checkpoint to also be justified
        let prev_epoch = last_justified.epoch.checked_sub(1)?;
        let prev_checkpoint = state.checkpoints.get(&prev_epoch)?;

        if prev_checkpoint.is_justified() {
            // Two consecutive justified - finalize the previous one
            let mut finalized = prev_checkpoint.clone();
            finalized.finalize();

            // Update state
            state.checkpoints.insert(prev_epoch, finalized.clone());
            state
                .finalized_blocks
                .insert(finalized.block_hash, finalized.block_height);
            state.last_finalized = Some(finalized.clone());

            Some(finalized)
        } else {
            None
        }
    }

    /// Send MarkFinalizedRequest to Block Storage
    async fn notify_finalization(&self, checkpoint: &Checkpoint) -> FinalityResult<()> {
        let source = {
            let state = self.state.read();
            // Get the source checkpoint (previous justified)
            let source_epoch = checkpoint.epoch.saturating_sub(1);
            state
                .checkpoints
                .get(&source_epoch)
                .cloned()
                .unwrap_or_else(|| checkpoint.clone())
        };

        let proof = FinalityProof::new(
            &source,
            checkpoint,
            BlsSignature::default(), // TODO: Aggregate actual signatures
            BitVec::new(),
            checkpoint.attested_stake,
            checkpoint.total_stake,
        );

        let request = MarkFinalizedRequest {
            correlation_id: Uuid::new_v4(),
            block_hash: checkpoint.block_hash,
            block_height: checkpoint.block_height,
            finalized_epoch: checkpoint.epoch,
            finality_proof: proof,
        };

        self.block_storage.mark_finalized(request).await
    }

    /// Get or create checkpoint for epoch
    fn get_or_create_checkpoint(
        &self,
        state: &mut FinalityServiceState,
        epoch: u64,
        block_hash: Hash,
        block_height: u64,
        total_stake: u128,
    ) -> Checkpoint {
        state
            .checkpoints
            .entry(epoch)
            .or_insert_with(|| {
                Checkpoint::new(epoch, block_hash, block_height).with_total_stake(total_stake)
            })
            .clone()
    }
}

#[async_trait]
impl<B, V, S> FinalityApi for FinalityService<B, V, S>
where
    B: BlockStorageGateway + 'static,
    V: AttestationVerifier + 'static,
    S: ValidatorSetProvider + 'static,
{
    async fn process_attestations(
        &self,
        attestations: Vec<Attestation>,
    ) -> FinalityResult<AttestationResult> {
        // Check circuit breaker
        {
            let state = self.state.read();
            if state.circuit_breaker.is_halted() {
                return Err(FinalityError::SystemHalted);
            }
        }

        if attestations.is_empty() {
            return Ok(AttestationResult::empty());
        }

        let mut accepted = 0usize;
        let mut rejected = 0usize;
        let mut new_justified = None;
        let mut new_finalized = None;

        // Get epoch from first attestation
        let epoch = attestations[0].target_checkpoint.epoch;

        // Get validator set for this epoch
        let validators = self
            .validator_provider
            .get_validator_set_at_epoch(epoch)
            .await?;
        let total_stake = validators.total_stake();

        // Process each attestation
        for attestation in &attestations {
            match self
                .process_single_attestation(attestation, &validators)
                .await
            {
                Ok(Some(stake)) => {
                    let mut state = self.state.write();

                    // Record attestation for slashing detection
                    self.record_attestation(&mut state, attestation);

                    // Get or create checkpoint
                    let target = &attestation.target_checkpoint;
                    let _checkpoint = self.get_or_create_checkpoint(
                        &mut state,
                        target.epoch,
                        target.block_hash,
                        0, // Height unknown from attestation
                        total_stake,
                    );

                    // Get or create aggregated attestations
                    let agg = state.attestations.entry(target.clone()).or_insert_with(|| {
                        AggregatedAttestations::new(
                            attestation.source_checkpoint.clone(),
                            target.clone(),
                            validators.len(),
                        )
                    });

                    // Check if already attested
                    if let Some(idx) = validators.get_index(&attestation.validator_id) {
                        if !agg.has_attested(idx) {
                            agg.add_attestation(attestation.clone(), idx, stake);

                            // Update checkpoint stake and check justification
                            let target_epoch = target.epoch;
                            let (justified, cp_clone) =
                                if let Some(cp) = state.checkpoints.get_mut(&target_epoch) {
                                    cp.add_attestation_stake(stake);
                                    let is_justified = cp.try_justify();
                                    (is_justified, Some(cp.clone()))
                                } else {
                                    (false, None)
                                };

                            if justified {
                                if let Some(cp) = cp_clone {
                                    state.last_justified = Some(cp.clone());
                                    new_justified = Some(cp);
                                }
                            }

                            accepted += 1;
                        } else {
                            rejected += 1; // Duplicate attestation
                        }
                    } else {
                        rejected += 1;
                    }
                }
                Ok(None) => rejected += 1,
                Err(_) => rejected += 1,
            }
        }

        // Check for finalization
        {
            let mut state = self.state.write();
            if let Some(finalized) = self.check_finalization(&mut state) {
                new_finalized = Some(finalized);

                // Reset inactivity counter on successful finalization
                state.epochs_without_finality = 0;

                // Update circuit breaker
                state
                    .circuit_breaker
                    .process_event(FinalityEvent::FinalityAchieved);
            } else {
                // Track epochs without finality
                let current_epoch = epoch;
                if state.current_epoch != current_epoch {
                    state.current_epoch = current_epoch;
                    state.epochs_without_finality += 1;

                    // Check if inactivity leak should trigger
                    if state.is_inactivity_leak_active(&self.config) {
                        tracing::warn!(
                            "INACTIVITY LEAK ACTIVE: {} epochs without finality",
                            state.epochs_without_finality
                        );
                    }
                }
            }
        }

        // Notify block storage if finalized
        if let Some(ref finalized) = new_finalized {
            if let Err(e) = self.notify_finalization(finalized).await {
                tracing::error!("Failed to notify finalization: {:?}", e);

                let mut state = self.state.write();
                state
                    .circuit_breaker
                    .process_event(FinalityEvent::FinalityFailed);
            }
        }

        Ok(AttestationResult {
            accepted,
            rejected,
            new_justified,
            new_finalized,
        })
    }

    async fn is_finalized(&self, block_hash: Hash) -> bool {
        self.state.read().finalized_blocks.contains_key(&block_hash)
    }

    async fn get_last_finalized(&self) -> Option<Checkpoint> {
        self.state.read().last_finalized.clone()
    }

    async fn get_state(&self) -> FinalityState {
        self.state.read().circuit_breaker.state()
    }

    async fn reset_from_halted(&self) -> FinalityResult<()> {
        let mut state = self.state.write();
        if !state.circuit_breaker.is_halted() {
            return Ok(());
        }

        state
            .circuit_breaker
            .process_event(FinalityEvent::ManualIntervention);
        Ok(())
    }

    async fn get_finality_lag(&self) -> u64 {
        let state = self.state.read();
        let finalized_height = state
            .last_finalized
            .as_ref()
            .map(|c| c.block_height)
            .unwrap_or(0);

        state.current_height.saturating_sub(finalized_height)
    }

    async fn get_current_epoch(&self) -> u64 {
        self.state.read().current_epoch
    }

    async fn get_checkpoint(&self, epoch: u64) -> Option<Checkpoint> {
        self.state.read().checkpoints.get(&epoch).cloned()
    }

    async fn get_epochs_without_finality(&self) -> u64 {
        self.state.read().epochs_without_finality
    }

    async fn is_inactivity_leak_active(&self) -> bool {
        self.state.read().is_inactivity_leak_active(&self.config)
    }

    async fn get_slashable_offenses(&self) -> Vec<crate::ports::inbound::SlashableOffenseInfo> {
        self.state
            .read()
            .slashable_offenses
            .iter()
            .map(|o| crate::ports::inbound::SlashableOffenseInfo {
                validator_id: o.validator_id.clone(),
                offense_type: match o.offense_type {
                    SlashableOffenseType::DoubleVote => {
                        crate::ports::inbound::SlashableOffenseType::DoubleVote
                    }
                    SlashableOffenseType::SurroundVote => {
                        crate::ports::inbound::SlashableOffenseType::SurroundVote
                    }
                },
                detected_epoch: o.detected_epoch,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // Mock implementations for testing
    struct MockBlockStorage {
        called: AtomicBool,
    }

    impl MockBlockStorage {
        fn new() -> Self {
            Self {
                called: AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl BlockStorageGateway for MockBlockStorage {
        async fn mark_finalized(&self, _request: MarkFinalizedRequest) -> FinalityResult<()> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    struct MockVerifier {
        always_valid: bool,
    }

    impl MockVerifier {
        fn new(always_valid: bool) -> Self {
            Self { always_valid }
        }
    }

    #[async_trait]
    impl AttestationVerifier for MockVerifier {
        fn verify_attestation(&self, _attestation: &Attestation) -> bool {
            self.always_valid
        }

        fn verify_aggregate(
            &self,
            _attestations: &AggregatedAttestations,
            _validators: &ValidatorSet,
        ) -> bool {
            self.always_valid
        }
    }

    struct MockValidatorProvider {
        validators: ValidatorSet,
    }

    impl MockValidatorProvider {
        fn new(count: usize, stake_each: u128) -> Self {
            let mut validators = ValidatorSet::new(1);
            for i in 0..count {
                let mut id = [0u8; 32];
                id[0] = i as u8;
                validators.add_validator(id.into(), stake_each);
            }
            Self { validators }
        }
    }

    #[async_trait]
    impl ValidatorSetProvider for MockValidatorProvider {
        async fn get_validator_set_at_epoch(&self, _epoch: u64) -> FinalityResult<ValidatorSet> {
            Ok(self.validators.clone())
        }

        async fn get_validator_stake(
            &self,
            validator_id: &crate::domain::ValidatorId,
            _epoch: u64,
        ) -> FinalityResult<u128> {
            self.validators
                .get_stake(validator_id)
                .ok_or(FinalityError::UnknownValidator {
                    validator_id: validator_id.0,
                })
        }

        async fn get_total_active_stake(&self, _epoch: u64) -> FinalityResult<u128> {
            Ok(self.validators.total_stake())
        }
    }

    fn create_test_service(
    ) -> FinalityService<MockBlockStorage, MockVerifier, MockValidatorProvider> {
        FinalityService::new(
            FinalityConfig::default(),
            Arc::new(MockBlockStorage::new()),
            Arc::new(MockVerifier::new(true)),
            Arc::new(MockValidatorProvider::new(100, 100)),
        )
    }

    #[tokio::test]
    async fn test_circuit_breaker_halted_blocks_processing() {
        let service = create_test_service();

        // Force halted state
        {
            let mut state = service.state.write();
            state
                .circuit_breaker
                .process_event(FinalityEvent::FinalityFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
        }

        let result = service.process_attestations(vec![]).await;
        assert!(matches!(result, Err(FinalityError::SystemHalted)));
    }

    #[tokio::test]
    async fn test_reset_from_halted() {
        let service = create_test_service();

        // Force halted state
        {
            let mut state = service.state.write();
            state
                .circuit_breaker
                .process_event(FinalityEvent::FinalityFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
        }

        assert!(service.get_state().await == FinalityState::HaltedAwaitingIntervention);

        service.reset_from_halted().await.unwrap();

        assert!(service.get_state().await == FinalityState::Running);
    }

    #[tokio::test]
    async fn test_empty_attestations() {
        let service = create_test_service();

        let result = service.process_attestations(vec![]).await.unwrap();
        assert_eq!(result.accepted, 0);
        assert_eq!(result.rejected, 0);
    }
}
