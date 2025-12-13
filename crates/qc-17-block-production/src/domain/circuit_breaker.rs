//! Circuit Breaker for Downstream Subsystem Resilience
//!
//! Implements the circuit breaker pattern to prevent cascading failures
//! when downstream subsystems (Mempool, State, Consensus) become unhealthy.
//!
//! ## States
//!
//! - **Closed**: Normal operation, requests flow through
//! - **Open**: Subsystem unhealthy, requests fail fast
//! - **Half-Open**: Testing recovery, limited requests allowed
//!
//! ## Usage
//!
//! ```ignore
//! let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
//!
//! // Before making request
//! if cb.should_allow("mempool") {
//!     match make_request().await {
//!         Ok(result) => cb.record_success("mempool"),
//!         Err(_) => cb.record_failure("mempool"),
//!     }
//! } else {
//!     // Circuit is open, fail fast
//! }
//! ```

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Circuit breaker configuration
#[derive(Clone, Debug)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Number of successes in half-open before closing
    pub success_threshold: u32,
    /// Duration to wait before transitioning from Open to Half-Open
    pub open_timeout: Duration,
    /// Enable/disable circuit breaker
    pub enabled: bool,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout: Duration::from_secs(30),
            enabled: true,
        }
    }
}

/// Circuit breaker state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation
    Closed,
    /// Failing fast
    Open,
    /// Testing recovery
    HalfOpen,
}

/// Circuit state for a single subsystem
struct SubsystemCircuit {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    total_failures: u64,
    total_successes: u64,
}

impl SubsystemCircuit {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            total_failures: 0,
            total_successes: 0,
        }
    }
}

/// Circuit breaker manager for multiple downstream subsystems
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    circuits: RwLock<HashMap<String, SubsystemCircuit>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            circuits: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a request to the subsystem should be allowed
    pub fn should_allow(&self, subsystem: &str) -> bool {
        if !self.config.enabled {
            return true;
        }

        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits
            .entry(subsystem.to_string())
            .or_insert_with(SubsystemCircuit::new);

        match circuit.state {
            CircuitState::Closed | CircuitState::HalfOpen => true,
            CircuitState::Open => {
                self.try_transition_to_half_open(circuit, subsystem)
            }
        }
    }

    /// Try to transition from Open to HalfOpen if timeout elapsed
    fn try_transition_to_half_open(&self, circuit: &mut SubsystemCircuit, subsystem: &str) -> bool {
        let Some(last_failure) = circuit.last_failure_time else {
            return false;
        };
        
        if last_failure.elapsed() < self.config.open_timeout {
            return false;
        }

        // Transition to half-open
        circuit.state = CircuitState::HalfOpen;
        circuit.success_count = 0;
        tracing::info!(
            "[circuit-breaker] {} transitioning to half-open",
            subsystem
        );
        true
    }

    /// Record a successful request
    pub fn record_success(&self, subsystem: &str) {
        if !self.config.enabled {
            return;
        }

        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits
            .entry(subsystem.to_string())
            .or_insert_with(SubsystemCircuit::new);

        circuit.total_successes += 1;

        match circuit.state {
            CircuitState::Closed => {
                // Reset failure count on success
                circuit.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                circuit.success_count += 1;
                if circuit.success_count >= self.config.success_threshold {
                    // Transition to closed
                    circuit.state = CircuitState::Closed;
                    circuit.failure_count = 0;
                    circuit.success_count = 0;
                    tracing::info!(
                        "[circuit-breaker] {} recovered, transitioning to closed",
                        subsystem
                    );
                }
            }
            CircuitState::Open => {
                // Should not happen (requests blocked in Open state)
            }
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, subsystem: &str) {
        if !self.config.enabled {
            return;
        }

        let mut circuits = self.circuits.write().unwrap();
        let circuit = circuits
            .entry(subsystem.to_string())
            .or_insert_with(SubsystemCircuit::new);

        circuit.total_failures += 1;
        circuit.failure_count += 1;
        circuit.last_failure_time = Some(Instant::now());

        match circuit.state {
            CircuitState::Closed => {
                if circuit.failure_count >= self.config.failure_threshold {
                    // Transition to open
                    circuit.state = CircuitState::Open;
                    tracing::warn!(
                        "[circuit-breaker] {} opened after {} failures",
                        subsystem,
                        circuit.failure_count
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open returns to open
                circuit.state = CircuitState::Open;
                circuit.success_count = 0;
                tracing::warn!(
                    "[circuit-breaker] {} failed in half-open, returning to open",
                    subsystem
                );
            }
            CircuitState::Open => {
                // Already open, just update failure time
            }
        }
    }

    /// Get the current state of a subsystem's circuit
    pub fn get_state(&self, subsystem: &str) -> CircuitState {
        let circuits = self.circuits.read().unwrap();
        circuits
            .get(subsystem)
            .map(|c| c.state)
            .unwrap_or(CircuitState::Closed)
    }

    /// Get statistics for all circuits
    pub fn get_stats(&self) -> Vec<CircuitStats> {
        let circuits = self.circuits.read().unwrap();
        circuits
            .iter()
            .map(|(name, circuit)| CircuitStats {
                subsystem: name.clone(),
                state: circuit.state,
                failure_count: circuit.failure_count,
                success_count: circuit.success_count,
                total_failures: circuit.total_failures,
                total_successes: circuit.total_successes,
            })
            .collect()
    }

    /// Reset a specific circuit
    pub fn reset(&self, subsystem: &str) {
        let mut circuits = self.circuits.write().unwrap();
        if let Some(circuit) = circuits.get_mut(subsystem) {
            circuit.state = CircuitState::Closed;
            circuit.failure_count = 0;
            circuit.success_count = 0;
            tracing::info!("[circuit-breaker] {} manually reset", subsystem);
        }
    }
}

/// Statistics for a circuit
#[derive(Clone, Debug)]
pub struct CircuitStats {
    /// Subsystem name
    pub subsystem: String,
    /// Current state
    pub state: CircuitState,
    /// Recent failure count
    pub failure_count: u32,
    /// Recent success count (in half-open)
    pub success_count: u32,
    /// Total failures since start
    pub total_failures: u64,
    /// Total successes since start
    pub total_successes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_starts_closed() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.get_state("mempool"), CircuitState::Closed);
        assert!(cb.should_allow("mempool"));
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Record failures
        cb.record_failure("mempool");
        cb.record_failure("mempool");
        assert_eq!(cb.get_state("mempool"), CircuitState::Closed);

        cb.record_failure("mempool");
        assert_eq!(cb.get_state("mempool"), CircuitState::Open);
        assert!(!cb.should_allow("mempool"));
    }

    #[test]
    fn test_circuit_transitions_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            open_timeout: Duration::from_millis(10),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        cb.record_failure("mempool");
        assert_eq!(cb.get_state("mempool"), CircuitState::Open);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(15));

        // Should transition to half-open and allow
        assert!(cb.should_allow("mempool"));
        assert_eq!(cb.get_state("mempool"), CircuitState::HalfOpen);
    }

    #[test]
    fn test_circuit_closes_after_successes_in_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_timeout: Duration::from_millis(10),
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure("mempool");

        // Wait and transition to half-open
        std::thread::sleep(Duration::from_millis(15));
        cb.should_allow("mempool");

        // Record successes
        cb.record_success("mempool");
        assert_eq!(cb.get_state("mempool"), CircuitState::HalfOpen);

        cb.record_success("mempool");
        assert_eq!(cb.get_state("mempool"), CircuitState::Closed);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        cb.record_failure("mempool");
        cb.record_failure("mempool");
        cb.record_success("mempool"); // Should reset

        cb.record_failure("mempool");
        cb.record_failure("mempool");
        // Should still be closed (not 3 consecutive failures)
        assert_eq!(cb.get_state("mempool"), CircuitState::Closed);
    }

    #[test]
    fn test_disabled_circuit_breaker() {
        let config = CircuitBreakerConfig {
            enabled: false,
            failure_threshold: 1,
            ..Default::default()
        };
        let cb = CircuitBreaker::new(config);

        cb.record_failure("mempool");
        assert!(cb.should_allow("mempool")); // Always allows when disabled
    }

    #[test]
    fn test_multiple_subsystems() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            ..Default::default()
        });

        cb.record_failure("mempool");
        cb.record_failure("mempool");

        cb.record_failure("state");

        assert_eq!(cb.get_state("mempool"), CircuitState::Open);
        assert_eq!(cb.get_state("state"), CircuitState::Closed);
    }
}
