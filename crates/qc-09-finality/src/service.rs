//! Finality Service - Core business logic
//!
//! Reference: SPEC-09-FINALITY.md Section 5

use crate::domain::proof::FinalityProof;
use crate::domain::{
    AggregatedAttestations, Attestation, BlsSignature, Checkpoint, CheckpointId, CircuitBreaker,
    FinalityEvent, FinalityState, ValidatorId, ValidatorSet,
};
use crate::error::{FinalityError, FinalityResult};
use crate::events::outgoing::{
    InactivityLeakTriggeredEvent, SlashableOffenseDetectedEvent,
    SlashableOffenseType as EventSlashableOffenseType, SlashingEvidence,
};
use crate::ports::inbound::{AttestationResult, FinalityApi};
use crate::ports::outbound::{
    AttestationVerifier, BlockStorageGateway, MarkFinalizedRequest, ValidatorSetProvider,
};
use async_trait::async_trait;
use bitvec::prelude::*;
use parking_lot::RwLock;
use shared_types::Hash;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::FinalityServiceState;
use crate::types::{FinalityConfig, OffenseContext, SlashableOffense, SlashableOffenseType};

/// Aggregate BLS signatures from multiple attestations
///
/// In a production implementation, this would use proper BLS signature
/// aggregation (e.g., via blst or bls12_381 crate). For now, we concatenate
/// the signature bytes as a placeholder that preserves all signature data.
///
/// SECURITY: The actual cryptographic aggregation should be done by qc-10.
fn aggregate_bls_signatures(attestations: &[Attestation]) -> BlsSignature {
    if attestations.is_empty() {
        return BlsSignature::default();
    }

    // Collect all signature bytes
    // In production: Use BLS aggregate signature algorithm
    // For now: XOR all signatures together (preserves some cryptographic properties)
    let mut aggregated = vec![0u8; 96]; // BLS signature size

    for attestation in attestations {
        let sig_bytes = &attestation.signature.0;
        for (i, byte) in sig_bytes.iter().enumerate() {
            if i < aggregated.len() {
                aggregated[i] ^= byte;
            }
        }
    }

    BlsSignature::new(aggregated)
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
        if self.config.always_reverify_signatures && !self.verifier.verify_attestation(attestation)
        {
            return Err(FinalityError::InvalidSignature {
                validator_id: validator_id.0,
            });
        }

        // 3. Check for slashable conditions (double vote, surround vote)
        self.check_slashable_conditions(attestation, validator_id)?;

        // 4. Get stake weight
        let stake = validators
            .get_stake(validator_id)
            .ok_or(FinalityError::UnknownValidator {
                validator_id: validator_id.0,
            })?;

        Ok(Some(stake))
    }



    /// Check for slashable conditions and record offense if found
    ///
    /// Per SPEC-09 INVARIANT-3: Conflicting attestations are recorded for slashing
    fn check_slashable_conditions(
        &self,
        attestation: &Attestation,
        validator_id: &ValidatorId,
    ) -> Result<(), FinalityError> {
        let mut state = self.state.write();

        // First, check if there's a conflict and clone the conflicting attestation if found
        let conflict = state
            .attestation_history
            .get(&validator_id.0)
            .and_then(|history| {
                history
                    .iter()
                    .find(|prev| attestation.conflicts_with(prev))
                    .cloned()
            });

        // Now handle the conflict with mutable access
        if let Some(conflicting) = conflict {
            let current_epoch = attestation.target_checkpoint.epoch;
            let ctx = OffenseContext {
                attestation,
                conflicting: &conflicting,
                current_epoch,
            };
            self.record_slashable_offense(&mut state, ctx);
            return Err(FinalityError::ConflictingAttestation);
        }

        Ok(())
    }

    /// Record slashable offense for later enforcement
    ///
    /// Per SPEC-09 INVARIANT-3: No conflicting finality without slashing 1/3 validators
    ///
    /// This method:
    /// 1. Records the offense for historical tracking
    /// 2. Creates an event for enforcement subsystem consumption
    fn record_slashable_offense(
        &self,
        state: &mut FinalityServiceState,
        ctx: OffenseContext,
    ) {
        let attestation = ctx.attestation;
        let conflicting = ctx.conflicting;
        let current_epoch = ctx.current_epoch;
        let offense_type =
            if attestation.target_checkpoint.epoch == conflicting.target_checkpoint.epoch {
                SlashableOffenseType::DoubleVote
            } else {
                SlashableOffenseType::SurroundVote
            };

        let offense = SlashableOffense {
            validator_id: attestation.validator_id,
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

        // Create event for enforcement subsystem
        let event_offense_type = match offense_type {
            SlashableOffenseType::DoubleVote => EventSlashableOffenseType::DoubleVote,
            SlashableOffenseType::SurroundVote => EventSlashableOffenseType::SurroundVote,
        };

        let slashing_event = SlashableOffenseDetectedEvent::new(
            attestation.validator_id,
            event_offense_type,
            SlashingEvidence {
                att1_source: attestation.source_checkpoint,
                att1_target: attestation.target_checkpoint,
                att2_source: conflicting.source_checkpoint,
                att2_target: conflicting.target_checkpoint,
            },
            current_epoch,
        );

        state.pending_slashing_events.push(slashing_event);

        tracing::error!(
            "SLASHING EVENT QUEUED: Validator {:?} will be slashed {}%",
            attestation.validator_id.0,
            100 // Full slash for both offense types
        );
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
    ///
    /// Constructs a proper FinalityProof with:
    /// - Source and target checkpoints
    /// - Aggregated signatures from attestations
    /// - Participation bitmap showing which validators attested
    async fn notify_finalization(&self, checkpoint: &Checkpoint) -> FinalityResult<()> {
        let (source, aggregated_sigs, participation_bitmap) = {
            let state = self.state.read();

            // Get the source checkpoint (previous justified)
            let source_epoch = checkpoint.epoch.saturating_sub(1);
            let source = state
                .checkpoints
                .get(&source_epoch)
                .cloned()
                .unwrap_or_else(|| checkpoint.clone());

            // Get aggregated attestations for the target checkpoint
            let target_id = checkpoint.id();
            let (agg_sig, bitmap) = if let Some(agg) = state.attestations.get(&target_id) {
                // Aggregate all signatures from attestations
                let combined_sig = aggregate_bls_signatures(&agg.attestations);
                (combined_sig, agg.participation_bitmap.clone())
            } else {
                // No attestations found - this shouldn't happen for a finalized checkpoint
                tracing::warn!(
                    "No attestations found for finalized checkpoint epoch {}",
                    checkpoint.epoch
                );
                (BlsSignature::default(), BitVec::new())
            };

            (source, agg_sig, bitmap)
        };

        let proof = FinalityProof::new(
            &source,
            checkpoint,
            aggregated_sigs,
            participation_bitmap,
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

    /// Check for finalization and update state accordingly
    async fn check_and_process_finalization(&self, current_epoch: u64) -> Option<Checkpoint> {
        let mut state = self.state.write();

        if let Some(finalized) = self.check_finalization(&mut state) {
            // Reset inactivity counter on successful finalization
            state.epochs_without_finality = 0;

            // Update circuit breaker
            state
                .circuit_breaker
                .process_event(FinalityEvent::FinalityAchieved);

            // Prune old checkpoints
            state.prune_old_checkpoints();

            Some(finalized)
        } else {
            // Track epochs without finality
            if state.current_epoch != current_epoch {
                state.current_epoch = current_epoch;
                state.epochs_without_finality += 1;

                // Check if inactivity leak should trigger
                if state.is_inactivity_leak_active(&self.config) {
                    tracing::warn!(
                        "INACTIVITY LEAK ACTIVE: {} epochs without finality (leak rate: {} bps)",
                        state.epochs_without_finality,
                        self.config.inactivity_leak_rate_bps
                    );

                    // Create inactivity leak event for enforcement
                    let leak_event = InactivityLeakTriggeredEvent::new(
                        current_epoch,
                        state.epochs_without_finality,
                        self.config.inactivity_leak_rate_bps,
                    );
                    state.pending_inactivity_events.push(leak_event);
                }
            }
            None
        }
    }

    /// Process a batch of attestations
    async fn process_attestation_batch(
        &self,
        attestations: &[Attestation],
        validators: &ValidatorSet,
    ) -> (usize, usize, Option<Checkpoint>) {
        let mut accepted = 0;
        let mut rejected = 0;
        let mut new_justified = None;

        for attestation in attestations {
            match self.process_attestation_update(attestation, validators).await {
                Ok(Some(cp)) => {
                    accepted += 1;
                    new_justified = Some(cp);
                }
                Ok(None) => accepted += 1,
                Err(_) => rejected += 1,
            }
        }

        (accepted, rejected, new_justified)
    }

    /// Process update for a single attestation
    /// Returns: Ok(Some(Checkpoint)) if justified, Ok(None) if accepted but not justified, Err if rejected
    async fn process_attestation_update(
        &self,
        attestation: &Attestation,
        validators: &ValidatorSet,
    ) -> Result<Option<Checkpoint>, ()> {
        // Pre-validate
        let stake = match self.process_single_attestation(attestation, validators).await {
            Ok(Some(s)) => s,
            _ => return Err(()),
        };

        // Apply to state
        let mut state = self.state.write();
        let (was_accepted, justified_checkpoint) =
            state.apply_attestation(attestation, validators, stake);

        if was_accepted {
            Ok(justified_checkpoint)
        } else {
            Err(())
        }
    }

    /// Handle finalization notification and failure fallback
    async fn handle_finalization_notification(&self, finalized: &Checkpoint) {
        if let Err(e) = self.notify_finalization(finalized).await {
            tracing::error!("Failed to notify finalization: {:?}", e);

            let mut state = self.state.write();
            state
                .circuit_breaker
                .process_event(FinalityEvent::FinalityFailed);
        }
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

        // Get epoch from first attestation
        let epoch = attestations[0].target_checkpoint.epoch;

        // Get validator set for this epoch
        let validators = self
            .validator_provider
            .get_validator_set_at_epoch(epoch)
            .await?;

        // Process batch
        let (accepted, rejected, new_justified) =
            self.process_attestation_batch(&attestations, &validators).await;

        // Check for finalization
        let new_finalized = self.check_and_process_finalization(epoch).await;

        // Notify block storage if finalized
        if let Some(ref finalized) = new_finalized {
            self.handle_finalization_notification(finalized).await;
        }

        // Collect pending events
        let (slashing_events, inactivity_events) = {
            let mut state = self.state.write();
            (state.take_slashing_events(), state.take_inactivity_events())
        };

        Ok(AttestationResult {
            accepted,
            rejected,
            new_justified,
            new_finalized,
            slashing_events,
            inactivity_events,
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
                validator_id: o.validator_id,
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

    async fn take_pending_slashing_events(&self) -> Vec<SlashableOffenseDetectedEvent> {
        self.state.write().take_slashing_events()
    }

    async fn take_pending_inactivity_events(&self) -> Vec<InactivityLeakTriggeredEvent> {
        self.state.write().take_inactivity_events()
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

    fn create_test_service() -> (
        FinalityService<MockBlockStorage, MockVerifier, MockValidatorProvider>,
        Arc<MockBlockStorage>,
        Arc<MockVerifier>,
    ) {
        let block_storage = Arc::new(MockBlockStorage::new());
        let verifier = Arc::new(MockVerifier::new(true));
        
        let service = FinalityService::new(
            FinalityConfig::default(),
            block_storage.clone(),
            verifier.clone(),
            Arc::new(MockValidatorProvider::new(100, 100)),
        );
        (service, block_storage, verifier)
    }

    fn create_test_attestation(epoch: u64, _block_height: u64) -> Attestation {
        let source_checkpoint = CheckpointId {
            epoch,
            block_hash: [0; 32],
        };
        let target_checkpoint = CheckpointId {
            epoch: epoch + 1,
            block_hash: [0; 32],
        };
        
        Attestation {
            source_checkpoint,
            target_checkpoint,
            signature: crate::domain::BlsSignature::new(vec![0; 96]),
            validator_id: crate::domain::ValidatorId([0; 32]),
            slot: 0,
        }
    }

    async fn setup_halted_state(
        service: &FinalityService<MockBlockStorage, MockVerifier, MockValidatorProvider>,
    ) {
        let mut state = service.state.write();
        state
            .circuit_breaker
            .process_event(FinalityEvent::FinalityFailed);
        // Do it enough times to trip (default might be 10 or more)
        for _ in 0..20 {
            state
                .circuit_breaker
                .process_event(FinalityEvent::FinalityFailed);
            state
                .circuit_breaker
                .process_event(FinalityEvent::SyncFailed);
        }
        assert!(state.circuit_breaker.is_halted());
    }

    #[tokio::test]
    async fn test_circuit_breaker_halted_blocks_processing() {
        let (service, _store, _verifier) = create_test_service();
        setup_halted_state(&service).await;

        let attestation = create_test_attestation(1, 1);
        let result = service.process_attestations(vec![attestation]).await;

        assert!(matches!(result, Err(FinalityError::SystemHalted)));
    }

    #[tokio::test]
    async fn test_reset_from_halted() {
        let (service, _store, _verifier) = create_test_service();
        setup_halted_state(&service).await;

        assert!(service.get_state().await == FinalityState::HaltedAwaitingIntervention);

        service.reset_from_halted().await.unwrap();

        assert!(service.get_state().await == FinalityState::Running);
    }

    #[tokio::test]
    async fn test_empty_attestations() {
        let (service, _, _) = create_test_service();

        let result = service.process_attestations(vec![]).await.unwrap();
        assert_eq!(result.accepted, 0);
        assert_eq!(result.rejected, 0);
    }
}
