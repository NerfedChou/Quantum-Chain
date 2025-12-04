//! Events module for Finality subsystem
//!
//! Reference: SPEC-09-FINALITY.md Section 4

pub mod incoming;
pub mod outgoing;

pub use incoming::AttestationBatch;
pub use outgoing::{
    FinalityAchievedEvent, InactivityLeakTriggeredEvent, MarkFinalizedPayload,
    SlashableOffenseDetectedEvent, ValidatorInactivityPenaltyEvent,
};
