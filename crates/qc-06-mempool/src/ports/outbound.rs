//! Outbound (Driven) ports for the Mempool subsystem.
//!
//! These traits define dependencies on external systems that the Mempool
//! needs for operation.

use crate::domain::{Address, MempoolError, Timestamp};

/// State provider interface for transaction validation.
///
/// Provides balance and nonce information from Subsystem 4 (State Management).
pub trait StateProvider: Send + Sync {
    /// Checks if an account has sufficient balance for a transaction.
    ///
    /// # Arguments
    /// - `address`: The account address
    /// - `required`: The required balance (value + gas cost)
    ///
    /// # Returns
    /// - `Ok(true)`: Balance is sufficient
    /// - `Ok(false)`: Balance is insufficient
    /// - `Err`: State lookup failed
    fn check_balance(&self, address: &Address, required: u128) -> Result<bool, MempoolError>;

    /// Gets the expected next nonce for an account.
    ///
    /// # Arguments
    /// - `address`: The account address
    ///
    /// # Returns
    /// The next expected nonce (number of confirmed transactions)
    fn get_nonce(&self, address: &Address) -> Result<u64, MempoolError>;

    /// Gets the current balance for an account.
    fn get_balance(&self, address: &Address) -> Result<u128, MempoolError>;
}

/// Time source for consistent timestamp handling.
///
/// Abstracted to allow testing with deterministic time.
pub trait TimeSource: Send + Sync {
    /// Returns the current timestamp in milliseconds.
    fn now(&self) -> Timestamp;
}

/// Default system time source.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now(&self) -> Timestamp {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as Timestamp
    }
}

/// Mock state provider for testing.
#[cfg(test)]
pub struct MockStateProvider {
    balances: std::collections::HashMap<Address, u128>,
    nonces: std::collections::HashMap<Address, u64>,
}

#[cfg(test)]
impl MockStateProvider {
    pub fn new() -> Self {
        Self {
            balances: std::collections::HashMap::new(),
            nonces: std::collections::HashMap::new(),
        }
    }

    pub fn with_balance(mut self, address: Address, balance: u128) -> Self {
        self.balances.insert(address, balance);
        self
    }

    pub fn with_nonce(mut self, address: Address, nonce: u64) -> Self {
        self.nonces.insert(address, nonce);
        self
    }
}

#[cfg(test)]
impl StateProvider for MockStateProvider {
    fn check_balance(&self, address: &Address, required: u128) -> Result<bool, MempoolError> {
        let balance = self.balances.get(address).copied().unwrap_or(0);
        Ok(balance >= required)
    }

    fn get_nonce(&self, address: &Address) -> Result<u64, MempoolError> {
        Ok(self.nonces.get(address).copied().unwrap_or(0))
    }

    fn get_balance(&self, address: &Address) -> Result<u128, MempoolError> {
        Ok(self.balances.get(address).copied().unwrap_or(0))
    }
}

/// Mock time source for testing.
#[cfg(test)]
pub struct MockTimeSource {
    time: std::sync::atomic::AtomicU64,
}

#[cfg(test)]
impl MockTimeSource {
    pub fn new(initial: Timestamp) -> Self {
        Self {
            time: std::sync::atomic::AtomicU64::new(initial),
        }
    }

    pub fn advance(&self, ms: u64) {
        self.time.fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn set(&self, time: Timestamp) {
        self.time.store(time, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(test)]
impl TimeSource for MockTimeSource {
    fn now(&self) -> Timestamp {
        self.time.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_time_source() {
        let source = SystemTimeSource;
        let now = source.now();
        
        // Should be a reasonable timestamp (after year 2020)
        assert!(now > 1577836800000); // Jan 1, 2020 in ms
    }

    #[test]
    fn test_mock_state_provider() {
        let address: Address = [0xAA; 20];
        let provider = MockStateProvider::new()
            .with_balance(address, 1_000_000)
            .with_nonce(address, 5);

        assert!(provider.check_balance(&address, 500_000).unwrap());
        assert!(!provider.check_balance(&address, 2_000_000).unwrap());
        assert_eq!(provider.get_nonce(&address).unwrap(), 5);
        assert_eq!(provider.get_balance(&address).unwrap(), 1_000_000);
    }

    #[test]
    fn test_mock_time_source() {
        let source = MockTimeSource::new(1000);
        assert_eq!(source.now(), 1000);

        source.advance(500);
        assert_eq!(source.now(), 1500);

        source.set(3000);
        assert_eq!(source.now(), 3000);
    }
}
