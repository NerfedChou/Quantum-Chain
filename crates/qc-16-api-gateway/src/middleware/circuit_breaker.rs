//! Circuit breaker middleware for downstream subsystem resilience.
//!
//! Implements a circuit breaker pattern to prevent cascading failures when
//! downstream subsystems become unhealthy. This protects the API Gateway from
//! resource exhaustion when internal services are unavailable.
//!
//! # Circuit Breaker States
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    CIRCUIT BREAKER STATE MACHINE                    │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                     │
//! │                    success                                          │
//! │            ┌─────────────────────┐                                  │
//! │            │                     │                                  │
//! │            ▼                     │                                  │
//! │      ┌──────────┐          ┌──────────┐          ┌──────────┐       │
//! │      │  CLOSED  │ ───────► │   OPEN   │ ───────► │HALF-OPEN │       │
//! │      │ (normal) │ failures │ (reject) │  timeout │  (probe) │       │
//! │      └──────────┘          └──────────┘          └──────────┘       │
//! │            ▲                                           │            │
//! │            │                                           │            │
//! │            └───────────────────────────────────────────┘            │
//! │                           success                                   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Configuration
//!
//! - `failure_threshold`: Number of failures before opening circuit (default: 5)
//! - `success_threshold`: Number of successes in half-open before closing (default: 3)
//! - `timeout`: Time before transitioning from open to half-open (default: 30s)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tracing::{debug, info, warn};

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Circuit is open - requests are rejected immediately
    Open,
    /// Testing if service is healthy - allows limited requests
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open state before closing
    pub success_threshold: u32,
    /// Duration before half-open from open state
    pub open_timeout: Duration,
    /// Duration to track failure rate
    pub failure_window: Duration,
    /// Enable circuit breaker
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout: Duration::from_secs(30),
            failure_window: Duration::from_secs(60),
            enabled: true,
        }
    }
}

/// Per-subsystem circuit breaker state
struct SubsystemCircuit {
    /// Current state
    state: CircuitState,
    /// Failure count in current window
    failure_count: AtomicU32,
    /// Success count in half-open state
    half_open_successes: AtomicU32,
    /// Time when circuit was opened
    opened_at: Option<Instant>,
    /// Last state transition time
    last_transition: Instant,
    /// Total requests sent
    total_requests: AtomicU64,
    /// Total failures
    total_failures: AtomicU64,
}

impl SubsystemCircuit {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: AtomicU32::new(0),
            half_open_successes: AtomicU32::new(0),
            opened_at: None,
            last_transition: Instant::now(),
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
        }
    }
}

/// Circuit breaker manager for all subsystems
pub struct CircuitBreakerManager {
    /// Per-subsystem circuits
    circuits: RwLock<HashMap<String, SubsystemCircuit>>,
    /// Configuration
    config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            circuits: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Check if a request to the subsystem should be allowed
    ///
    /// Returns `true` if the request should proceed, `false` if the circuit is open.
    pub fn should_allow(&self, subsystem: &str) -> bool {
        if !self.config.enabled {
            return true;
        }

        // Get or create circuit for this subsystem
        let mut circuits = self.circuits.write();
        let circuit = circuits
            .entry(subsystem.to_string())
            .or_insert_with(SubsystemCircuit::new);

        circuit.total_requests.fetch_add(1, Ordering::Relaxed);

        match circuit.state {
            CircuitState::Closed => {
                // Normal operation - allow request
                true
            }
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(opened_at) = circuit.opened_at {
                    if opened_at.elapsed() >= self.config.open_timeout {
                        // Transition to half-open
                        info!(
                            subsystem = subsystem,
                            "Circuit breaker transitioning to half-open"
                        );
                        circuit.state = CircuitState::HalfOpen;
                        circuit.half_open_successes.store(0, Ordering::Relaxed);
                        circuit.last_transition = Instant::now();
                        true // Allow the probe request
                    } else {
                        // Still in timeout - reject
                        debug!(
                            subsystem = subsystem,
                            remaining_ms = (self.config.open_timeout - opened_at.elapsed()).as_millis(),
                            "Circuit breaker is open, rejecting request"
                        );
                        false
                    }
                } else {
                    // No opened_at time - shouldn't happen, but treat as closed
                    true
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests for probing
                true
            }
        }
    }

    /// Record a successful request
    pub fn record_success(&self, subsystem: &str) {
        if !self.config.enabled {
            return;
        }

        let mut circuits = self.circuits.write();
        if let Some(circuit) = circuits.get_mut(subsystem) {
            match circuit.state {
                CircuitState::Closed => {
                    // Reset failure count on success
                    circuit.failure_count.store(0, Ordering::Relaxed);
                }
                CircuitState::HalfOpen => {
                    // Count successes
                    let successes = circuit.half_open_successes.fetch_add(1, Ordering::Relaxed) + 1;
                    if successes >= self.config.success_threshold {
                        // Transition back to closed
                        info!(
                            subsystem = subsystem,
                            successes = successes,
                            "Circuit breaker closing after successful probes"
                        );
                        circuit.state = CircuitState::Closed;
                        circuit.failure_count.store(0, Ordering::Relaxed);
                        circuit.opened_at = None;
                        circuit.last_transition = Instant::now();
                    }
                }
                CircuitState::Open => {
                    // Shouldn't happen - we don't send requests when open
                }
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, subsystem: &str) {
        if !self.config.enabled {
            return;
        }

        let mut circuits = self.circuits.write();
        let circuit = circuits
            .entry(subsystem.to_string())
            .or_insert_with(SubsystemCircuit::new);

        circuit.total_failures.fetch_add(1, Ordering::Relaxed);

        match circuit.state {
            CircuitState::Closed => {
                // Count failures
                let failures = circuit.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
                if failures >= self.config.failure_threshold {
                    // Transition to open
                    warn!(
                        subsystem = subsystem,
                        failures = failures,
                        threshold = self.config.failure_threshold,
                        timeout_secs = self.config.open_timeout.as_secs(),
                        "Circuit breaker opening due to failures"
                    );
                    circuit.state = CircuitState::Open;
                    circuit.opened_at = Some(Instant::now());
                    circuit.last_transition = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                // Failure in half-open - go back to open
                warn!(
                    subsystem = subsystem,
                    "Circuit breaker reopening after probe failure"
                );
                circuit.state = CircuitState::Open;
                circuit.opened_at = Some(Instant::now());
                circuit.half_open_successes.store(0, Ordering::Relaxed);
                circuit.last_transition = Instant::now();
            }
            CircuitState::Open => {
                // Already open - update opened_at to extend timeout
                circuit.opened_at = Some(Instant::now());
            }
        }
    }

    /// Get the current state of a subsystem's circuit breaker
    pub fn get_state(&self, subsystem: &str) -> CircuitState {
        let circuits = self.circuits.read();
        circuits
            .get(subsystem)
            .map(|c| c.state)
            .unwrap_or(CircuitState::Closed)
    }

    /// Get statistics for all circuits
    pub fn get_stats(&self) -> Vec<CircuitStats> {
        let circuits = self.circuits.read();
        circuits
            .iter()
            .map(|(subsystem, circuit)| CircuitStats {
                subsystem: subsystem.clone(),
                state: circuit.state,
                failure_count: circuit.failure_count.load(Ordering::Relaxed),
                total_requests: circuit.total_requests.load(Ordering::Relaxed),
                total_failures: circuit.total_failures.load(Ordering::Relaxed),
                last_transition_ms: circuit.last_transition.elapsed().as_millis() as u64,
                time_in_state_ms: if let Some(opened_at) = circuit.opened_at {
                    opened_at.elapsed().as_millis() as u64
                } else {
                    circuit.last_transition.elapsed().as_millis() as u64
                },
            })
            .collect()
    }

    /// Reset a specific circuit (for admin purposes)
    pub fn reset(&self, subsystem: &str) {
        let mut circuits = self.circuits.write();
        if let Some(circuit) = circuits.get_mut(subsystem) {
            info!(subsystem = subsystem, "Circuit breaker manually reset");
            circuit.state = CircuitState::Closed;
            circuit.failure_count.store(0, Ordering::Relaxed);
            circuit.half_open_successes.store(0, Ordering::Relaxed);
            circuit.opened_at = None;
            circuit.last_transition = Instant::now();
        }
    }

    /// Reset all circuits
    pub fn reset_all(&self) {
        let mut circuits = self.circuits.write();
        for (subsystem, circuit) in circuits.iter_mut() {
            info!(subsystem = subsystem, "Circuit breaker manually reset");
            circuit.state = CircuitState::Closed;
            circuit.failure_count.store(0, Ordering::Relaxed);
            circuit.half_open_successes.store(0, Ordering::Relaxed);
            circuit.opened_at = None;
            circuit.last_transition = Instant::now();
        }
    }
}

/// Statistics for a circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitStats {
    pub subsystem: String,
    pub state: CircuitState,
    pub failure_count: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub last_transition_ms: u64,
    pub time_in_state_ms: u64,
}

impl serde::Serialize for CircuitStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CircuitStats", 7)?;
        state.serialize_field("subsystem", &self.subsystem)?;
        state.serialize_field("state", &self.state.to_string())?;
        state.serialize_field("failure_count", &self.failure_count)?;
        state.serialize_field("total_requests", &self.total_requests)?;
        state.serialize_field("total_failures", &self.total_failures)?;
        state.serialize_field("last_transition_ms", &self.last_transition_ms)?;
        state.serialize_field("time_in_state_ms", &self.time_in_state_ms)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            open_timeout: Duration::from_millis(100),
            failure_window: Duration::from_secs(60),
            enabled: true,
        }
    }

    #[test]
    fn test_circuit_starts_closed() {
        let manager = CircuitBreakerManager::new(test_config());
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_allows_when_closed() {
        let manager = CircuitBreakerManager::new(test_config());
        assert!(manager.should_allow("test-subsystem"));
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let manager = CircuitBreakerManager::new(test_config());

        // Record failures up to threshold
        for i in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
            
            if i < 2 {
                assert_eq!(manager.get_state("test-subsystem"), CircuitState::Closed);
            }
        }

        // Circuit should now be open
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Open);
    }

    #[test]
    fn test_circuit_rejects_when_open() {
        let mut config = test_config();
        config.open_timeout = Duration::from_secs(1000); // Long timeout
        let manager = CircuitBreakerManager::new(config);

        // Open the circuit
        for _ in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        // Should reject requests
        assert!(!manager.should_allow("test-subsystem"));
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let manager = CircuitBreakerManager::new(test_config());

        // Open the circuit
        for _ in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(150));

        // Next request should transition to half-open
        assert!(manager.should_allow("test-subsystem"));
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_closes_after_successes_in_half_open() {
        let manager = CircuitBreakerManager::new(test_config());

        // Open the circuit
        for _ in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        // Wait for timeout and trigger half-open
        std::thread::sleep(Duration::from_millis(150));
        manager.should_allow("test-subsystem");

        // Record successes
        manager.record_success("test-subsystem");
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::HalfOpen);
        
        manager.record_success("test-subsystem");
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_reopens_on_failure_in_half_open() {
        let manager = CircuitBreakerManager::new(test_config());

        // Open the circuit
        for _ in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        // Wait for timeout and trigger half-open
        std::thread::sleep(Duration::from_millis(150));
        manager.should_allow("test-subsystem");
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::HalfOpen);

        // Failure in half-open should reopen
        manager.record_failure("test-subsystem");
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Open);
    }

    #[test]
    fn test_reset_circuit() {
        let manager = CircuitBreakerManager::new(test_config());

        // Open the circuit
        for _ in 0..3 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Open);

        // Reset
        manager.reset("test-subsystem");
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Closed);
        assert!(manager.should_allow("test-subsystem"));
    }

    #[test]
    fn test_disabled_circuit_breaker() {
        let mut config = test_config();
        config.enabled = false;
        let manager = CircuitBreakerManager::new(config);

        // Record many failures
        for _ in 0..10 {
            manager.should_allow("test-subsystem");
            manager.record_failure("test-subsystem");
        }

        // Should still allow - circuit breaker is disabled
        assert!(manager.should_allow("test-subsystem"));
    }

    #[test]
    fn test_success_resets_failure_count() {
        let manager = CircuitBreakerManager::new(test_config());

        // Record some failures
        manager.should_allow("test-subsystem");
        manager.record_failure("test-subsystem");
        manager.should_allow("test-subsystem");
        manager.record_failure("test-subsystem");

        // Record a success
        manager.record_success("test-subsystem");

        // Record more failures - should need full threshold again
        manager.should_allow("test-subsystem");
        manager.record_failure("test-subsystem");
        manager.should_allow("test-subsystem");
        manager.record_failure("test-subsystem");

        // Should still be closed (only 2 failures after reset)
        assert_eq!(manager.get_state("test-subsystem"), CircuitState::Closed);
    }

    #[test]
    fn test_get_stats() {
        let manager = CircuitBreakerManager::new(test_config());

        manager.should_allow("subsystem-a");
        manager.record_success("subsystem-a");
        manager.should_allow("subsystem-b");
        manager.record_failure("subsystem-b");

        let stats = manager.get_stats();
        assert_eq!(stats.len(), 2);
    }
}
