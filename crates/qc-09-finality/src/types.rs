use crate::domain::{Attestation, ValidatorId};

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
    /// Inactivity leak rate per epoch (basis points, 100 = 1%)
    /// Applied to inactive validators when leak is active
    pub inactivity_leak_rate_bps: u32,
    /// Always re-verify signatures (zero-trust)
    pub always_reverify_signatures: bool,
}

impl Default for FinalityConfig {
    fn default() -> Self {
        Self {
            epoch_length: 32,
            justification_threshold_percent: 66,
            max_sync_attempts: 10,
            sync_timeout_secs: 5,
            inactivity_leak_epochs: 4,
            inactivity_leak_rate_bps: 100, // 1%
            always_reverify_signatures: false,
        }
    }
}

/// Slashable offense detected during attestation processing
#[derive(Clone, Debug)]
pub struct SlashableOffense {
    pub validator_id: ValidatorId,
    pub offense_type: SlashableOffenseType,
    pub attestation1: Attestation,
    pub attestation2: Attestation,
    pub detected_epoch: u64,
}

/// Context for recording a slashable offense
pub struct OffenseContext<'a> {
    pub attestation: &'a Attestation,
    pub conflicting: &'a Attestation,
    pub current_epoch: u64,
}

/// Type of slashable offense
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlashableOffenseType {
    /// Same target epoch, different target block
    DoubleVote,
    /// One attestation surrounds another
    SurroundVote,
}
