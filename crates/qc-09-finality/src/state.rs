use crate::domain::{
    AggregatedAttestations, Attestation, Checkpoint, CheckpointId, CircuitBreaker,
    ValidatorSet,
};
use crate::events::outgoing::{
    InactivityLeakTriggeredEvent, SlashableOffenseDetectedEvent,
};
use crate::types::{FinalityConfig, SlashableOffense};
use shared_types::Hash;
use std::collections::{HashMap, VecDeque};

pub struct FinalityServiceState {
    /// Circuit breaker for livelock prevention
    pub circuit_breaker: CircuitBreaker,
    /// Checkpoints by epoch
    pub checkpoints: HashMap<u64, Checkpoint>,
    /// Aggregated attestations by checkpoint
    pub attestations: HashMap<CheckpointId, AggregatedAttestations>,
    /// Finalized block hashes
    pub finalized_blocks: HashMap<Hash, u64>,
    /// Last finalized checkpoint
    pub last_finalized: Option<Checkpoint>,
    /// Last justified checkpoint
    pub last_justified: Option<Checkpoint>,
    /// Current epoch
    pub current_epoch: u64,
    /// Current head height
    pub current_height: u64,
    /// Epochs without finality (for inactivity leak)
    pub epochs_without_finality: u64,
    /// Attestation history for slashing detection (validator_id -> attestations)
    /// Uses VecDeque for O(1) removal from front when pruning old entries
    pub attestation_history: HashMap<[u8; 32], VecDeque<Attestation>>,
    /// Detected slashable offenses
    pub slashable_offenses: Vec<SlashableOffense>,
    /// Pending slashing events to be emitted
    pub pending_slashing_events: Vec<SlashableOffenseDetectedEvent>,
    /// Pending inactivity leak events
    pub pending_inactivity_events: Vec<InactivityLeakTriggeredEvent>,
    /// Maximum checkpoints to retain (pruning threshold)
    pub max_checkpoints: usize,
}

impl FinalityServiceState {
    pub fn new() -> Self {
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
            pending_slashing_events: Vec::new(),
            pending_inactivity_events: Vec::new(),
            max_checkpoints: 128, // Keep ~4 epochs worth at 32 blocks/epoch
        }
    }

    /// Check if inactivity leak should be triggered
    /// Reference: SPEC-09-FINALITY.md - inactivity_leak_epochs
    pub fn is_inactivity_leak_active(&self, config: &FinalityConfig) -> bool {
        self.epochs_without_finality >= config.inactivity_leak_epochs
    }

    /// Prune old checkpoints to prevent unbounded memory growth
    /// Keeps only the most recent max_checkpoints entries
    pub fn prune_old_checkpoints(&mut self) {
        if self.checkpoints.len() <= self.max_checkpoints {
            return;
        }

        // Find the minimum epoch to keep
        let last_finalized_epoch = self.last_finalized.as_ref().map(|c| c.epoch).unwrap_or(0);
        let min_keep_epoch = last_finalized_epoch.saturating_sub(2); // Keep 2 epochs before finalized

        // Remove old checkpoints
        self.checkpoints.retain(|epoch, _| *epoch >= min_keep_epoch);

        // Also prune attestations for removed checkpoints
        self.attestations.retain(|id, _| id.epoch >= min_keep_epoch);
    }

    /// Take and clear pending slashing events
    pub fn take_slashing_events(&mut self) -> Vec<SlashableOffenseDetectedEvent> {
        std::mem::take(&mut self.pending_slashing_events)
    }

    /// Take and clear pending inactivity events
    pub fn take_inactivity_events(&mut self) -> Vec<InactivityLeakTriggeredEvent> {
        std::mem::take(&mut self.pending_inactivity_events)
    }

    /// Record attestation in history for slashing detection
    pub fn record_attestation(&mut self, attestation: &Attestation) {
        let history = self
            .attestation_history
            .entry(attestation.validator_id.0)
            .or_default();

        // Keep only recent attestations (last 2 epochs worth)
        const MAX_HISTORY: usize = 64;
        while history.len() >= MAX_HISTORY {
            history.pop_front(); // O(1) removal from front
        }
        history.push_back(attestation.clone()); // O(1) insertion at back
    }

    /// Get or create checkpoint for epoch
    pub fn get_or_create_checkpoint(
        &mut self,
        target: &Checkpoint,
        total_stake: u128,
    ) -> Checkpoint {
        self.checkpoints
            .entry(target.epoch)
            .or_insert_with(|| {
                Checkpoint::new(target.epoch, target.block_hash, 0).with_total_stake(total_stake)
            })
            .clone()
    }

    /// Apply a validated attestation to the state
    /// Returns (accepted, new_justified_checkpoint)
    pub fn apply_attestation(
        &mut self,
        attestation: &Attestation,
        validators: &ValidatorSet,
        stake: u128,
    ) -> (bool, Option<Checkpoint>) {
        // Record attestation for slashing detection
        self.record_attestation(attestation);

        // Get or create checkpoint
        let target = &attestation.target_checkpoint;
        let temp_checkpoint = Checkpoint::new(target.epoch, target.block_hash, 0);
        let _checkpoint = self.get_or_create_checkpoint(&temp_checkpoint, validators.total_stake());

        // Get or create aggregated attestations
        let agg = self
            .attestations
            .entry(*target)
            .or_insert_with(|| {
                AggregatedAttestations::new(attestation.source_checkpoint, *target, validators.len())
            });

        // Check if already attested
        let idx = match validators.get_index(&attestation.validator_id) {
            Some(i) => i,
            None => return (false, None),
        };

        if agg.has_attested(idx) {
            return (false, None); // Duplicate attestation
        }

        agg.add_attestation(attestation.clone(), idx, stake);

        // Update checkpoint stake and check justification
        let target_epoch = target.epoch;
        if let Some(cp) = self.checkpoints.get_mut(&target_epoch) {
            cp.add_attestation_stake(stake);
            if cp.try_justify() {
                self.last_justified = Some(cp.clone());
                return (true, Some(cp.clone()));
            }
        }
        (true, None)
    }
}
