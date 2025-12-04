//! Circuit Breaker for livelock prevention
//!
//! Reference: SPEC-09-FINALITY.md Section 1.3, Architecture.md Section 5.4.1
//!
//! The circuit breaker prevents infinite retry loops when finality fails.
//! After 3 failed sync attempts, the system halts and requires manual intervention.

use serde::{Deserialize, Serialize};

/// Circuit breaker state
///
/// Reference: SPEC-09-FINALITY.md Section 1.3
///
/// State Machine:
/// ```text
/// [RUNNING] ──finality failed──→ [SYNC {attempt: 1}]
///     ↑                                │
///     │                                ├── sync success ──→ [RUNNING]
///     │                                │
///     │                                └── sync failed ──→ [SYNC {attempt: n+1}]
///     │                                                          │
///     │                                                          ↓
///     │                                              attempt >= 3? ──→ [HALTED]
///     │                                                                    │
///     └────────────────── manual intervention ─────────────────────────────┘
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum FinalityState {
    /// Normal operation - processing attestations and finalizing blocks
    #[default]
    Running,
    /// Attempting to sync due to finality failure
    Sync { attempt: u8 },
    /// Halted after max sync failures - requires manual intervention
    HaltedAwaitingIntervention,
}


/// Events that trigger circuit breaker state transitions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalityEvent {
    /// Finality process succeeded
    FinalityAchieved,
    /// Finality process failed (couldn't reach consensus)
    FinalityFailed,
    /// Sync attempt succeeded
    SyncSuccess,
    /// Sync attempt failed
    SyncFailed,
    /// Manual operator intervention
    ManualIntervention,
}

/// Circuit breaker for finality subsystem
///
/// Reference: Architecture.md Section 5.4.1 - Deterministic circuit breaker
///
/// INVARIANT-4: State transitions are deterministic and testable
#[derive(Debug)]
pub struct CircuitBreaker {
    state: FinalityState,
    max_sync_attempts: u8,
    /// Total finality failures since last success
    consecutive_failures: u64,
    /// Total manual interventions
    intervention_count: u64,
}

impl CircuitBreaker {
    /// Create new circuit breaker with default config
    pub fn new() -> Self {
        Self {
            state: FinalityState::Running,
            max_sync_attempts: 3,
            consecutive_failures: 0,
            intervention_count: 0,
        }
    }

    /// Create with custom max sync attempts
    pub fn with_max_attempts(max_sync_attempts: u8) -> Self {
        Self {
            state: FinalityState::Running,
            max_sync_attempts,
            consecutive_failures: 0,
            intervention_count: 0,
        }
    }

    /// Get current state
    pub fn state(&self) -> FinalityState {
        self.state
    }

    /// Check if system is running normally
    pub fn is_running(&self) -> bool {
        matches!(self.state, FinalityState::Running)
    }

    /// Check if system is halted
    pub fn is_halted(&self) -> bool {
        matches!(self.state, FinalityState::HaltedAwaitingIntervention)
    }

    /// Process an event and transition state
    ///
    /// INVARIANT-4: Deterministic state transitions
    /// Reference: SPEC-09-FINALITY.md Section 2.2
    pub fn process_event(&mut self, event: FinalityEvent) -> FinalityState {
        let new_state = self.next_state(event);

        // Update metrics
        match event {
            FinalityEvent::FinalityFailed | FinalityEvent::SyncFailed => {
                self.consecutive_failures += 1;
            }
            FinalityEvent::FinalityAchieved | FinalityEvent::SyncSuccess => {
                self.consecutive_failures = 0;
            }
            FinalityEvent::ManualIntervention => {
                self.intervention_count += 1;
                self.consecutive_failures = 0;
            }
        }

        self.state = new_state;
        new_state
    }

    /// Calculate next state based on current state and event
    ///
    /// INVARIANT-4: Pure, deterministic function
    fn next_state(&self, event: FinalityEvent) -> FinalityState {
        match (self.state, event) {
            // Running state transitions
            (FinalityState::Running, FinalityEvent::FinalityAchieved) => FinalityState::Running,
            (FinalityState::Running, FinalityEvent::FinalityFailed) => {
                FinalityState::Sync { attempt: 1 }
            }

            // Sync state transitions
            (FinalityState::Sync { .. }, FinalityEvent::SyncSuccess) => FinalityState::Running,
            (FinalityState::Sync { attempt }, FinalityEvent::SyncFailed) => {
                if attempt >= self.max_sync_attempts {
                    FinalityState::HaltedAwaitingIntervention
                } else {
                    FinalityState::Sync {
                        attempt: attempt + 1,
                    }
                }
            }

            // Halted state transitions
            (FinalityState::HaltedAwaitingIntervention, FinalityEvent::ManualIntervention) => {
                FinalityState::Running
            }

            // No-op transitions (stay in current state)
            (state, _) => state,
        }
    }

    /// Force state for testing/recovery
    #[cfg(test)]
    pub fn force_state(&mut self, state: FinalityState) {
        self.state = state;
    }

    /// Get consecutive failure count
    pub fn consecutive_failures(&self) -> u64 {
        self.consecutive_failures
    }

    /// Get intervention count
    pub fn intervention_count(&self) -> u64 {
        self.intervention_count
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_running_to_sync() {
        let mut cb = CircuitBreaker::new();
        assert!(cb.is_running());

        cb.process_event(FinalityEvent::FinalityFailed);
        assert_eq!(cb.state(), FinalityState::Sync { attempt: 1 });
    }

    #[test]
    fn test_circuit_breaker_sync_to_running() {
        let mut cb = CircuitBreaker::new();
        cb.process_event(FinalityEvent::FinalityFailed);
        assert_eq!(cb.state(), FinalityState::Sync { attempt: 1 });

        cb.process_event(FinalityEvent::SyncSuccess);
        assert!(cb.is_running());
    }

    #[test]
    fn test_circuit_breaker_max_attempts_to_halted() {
        let mut cb = CircuitBreaker::new();

        // First failure
        cb.process_event(FinalityEvent::FinalityFailed);
        assert_eq!(cb.state(), FinalityState::Sync { attempt: 1 });

        // Second failure
        cb.process_event(FinalityEvent::SyncFailed);
        assert_eq!(cb.state(), FinalityState::Sync { attempt: 2 });

        // Third failure
        cb.process_event(FinalityEvent::SyncFailed);
        assert_eq!(cb.state(), FinalityState::Sync { attempt: 3 });

        // Fourth failure -> HALTED
        cb.process_event(FinalityEvent::SyncFailed);
        assert!(cb.is_halted());
    }

    #[test]
    fn test_circuit_breaker_manual_reset() {
        let mut cb = CircuitBreaker::new();
        cb.force_state(FinalityState::HaltedAwaitingIntervention);
        assert!(cb.is_halted());

        cb.process_event(FinalityEvent::ManualIntervention);
        assert!(cb.is_running());
        assert_eq!(cb.intervention_count(), 1);
    }

    #[test]
    fn test_circuit_breaker_consecutive_failures() {
        let mut cb = CircuitBreaker::new();

        cb.process_event(FinalityEvent::FinalityFailed);
        assert_eq!(cb.consecutive_failures(), 1);

        cb.process_event(FinalityEvent::SyncFailed);
        assert_eq!(cb.consecutive_failures(), 2);

        cb.process_event(FinalityEvent::SyncSuccess);
        assert_eq!(cb.consecutive_failures(), 0);
    }

    #[test]
    fn test_circuit_breaker_halted_blocks_events() {
        let mut cb = CircuitBreaker::new();
        cb.force_state(FinalityState::HaltedAwaitingIntervention);

        // These events should not change halted state
        cb.process_event(FinalityEvent::FinalityAchieved);
        assert!(cb.is_halted());

        cb.process_event(FinalityEvent::SyncSuccess);
        assert!(cb.is_halted());

        // Only manual intervention resets
        cb.process_event(FinalityEvent::ManualIntervention);
        assert!(cb.is_running());
    }

    #[test]
    fn test_circuit_breaker_determinism() {
        // INVARIANT-4: Same inputs produce same outputs
        let mut cb1 = CircuitBreaker::new();
        let mut cb2 = CircuitBreaker::new();

        let events = vec![
            FinalityEvent::FinalityFailed,
            FinalityEvent::SyncFailed,
            FinalityEvent::SyncSuccess,
            FinalityEvent::FinalityFailed,
        ];

        for event in events {
            let state1 = cb1.process_event(event);
            let state2 = cb2.process_event(event);
            assert_eq!(state1, state2);
        }
    }
}
