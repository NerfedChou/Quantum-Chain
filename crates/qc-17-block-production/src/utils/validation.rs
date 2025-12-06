//! Transaction validation utilities

use crate::domain::TransactionCandidate;
use crate::error::{BlockProductionError, Result};
use std::collections::{HashMap, HashSet};

/// Trait for transaction validation
pub trait TransactionValidator {
    /// Validate a single transaction
    fn validate(&self, tx: &TransactionCandidate) -> Result<()>;
}

/// Validate transaction signature
pub struct SignatureValidator;

impl TransactionValidator for SignatureValidator {
    fn validate(&self, tx: &TransactionCandidate) -> Result<()> {
        if !tx.signature_valid {
            return Err(BlockProductionError::InvalidSignature);
        }
        Ok(())
    }
}

/// Validate transaction gas parameters
pub struct GasValidator {
    max_gas_limit: u64,
    min_gas_price: primitive_types::U256,
}

impl GasValidator {
    /// Creates a new gas validator with given limits
    pub fn new(max_gas_limit: u64, min_gas_price: primitive_types::U256) -> Self {
        Self {
            max_gas_limit,
            min_gas_price,
        }
    }
}

impl TransactionValidator for GasValidator {
    fn validate(&self, tx: &TransactionCandidate) -> Result<()> {
        if tx.gas_limit > self.max_gas_limit {
            return Err(BlockProductionError::GasLimitExceeded {
                limit: self.max_gas_limit,
                used: tx.gas_limit,
            });
        }

        if tx.gas_price < self.min_gas_price {
            return Err(BlockProductionError::InternalError(format!(
                "Gas price {} below minimum {}",
                tx.gas_price, self.min_gas_price
            )));
        }

        Ok(())
    }
}

/// Composite validator that runs multiple validators
pub struct CompositeValidator {
    validators: Vec<Box<dyn TransactionValidator + Send + Sync>>,
}

impl CompositeValidator {
    /// Creates a new composite validator
    pub fn new() -> Self {
        Self {
            validators: Vec::new(),
        }
    }

    /// Adds a validator to the chain
    pub fn add_validator<V: TransactionValidator + Send + Sync + 'static>(
        mut self,
        validator: V,
    ) -> Self {
        self.validators.push(Box::new(validator));
        self
    }

    /// Validates a batch of transactions
    pub fn validate_batch(&self, transactions: &[TransactionCandidate]) -> Result<()> {
        for tx in transactions {
            for validator in &self.validators {
                validator.validate(tx)?;
            }
        }
        Ok(())
    }
}

impl Default for CompositeValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Batch validation utilities
pub mod batch {
    use super::*;

    /// Check for duplicate transactions in a batch
    pub fn check_duplicates(transactions: &[Vec<u8>]) -> Result<()> {
        let mut seen = HashSet::new();

        for tx in transactions {
            if !seen.insert(tx) {
                return Err(BlockProductionError::InternalError(
                    "Duplicate transaction in batch".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Check nonce ordering for transactions from same sender
    pub fn check_nonce_chains(transactions: &[TransactionCandidate]) -> Result<()> {
        let mut sender_nonces: HashMap<[u8; 20], Vec<u64>> = HashMap::new();

        // Group by sender
        for tx in transactions {
            sender_nonces.entry(tx.from).or_default().push(tx.nonce);
        }

        // Verify sequential nonces
        for (address, mut nonces) in sender_nonces {
            nonces.sort_unstable();

            for window in nonces.windows(2) {
                if window[1] != window[0] + 1 {
                    return Err(BlockProductionError::NonceMismatch {
                        address: hex::encode(address),
                        expected: window[0] + 1,
                        actual: window[1],
                    });
                }
            }
        }

        Ok(())
    }

    /// Calculate total gas used by transactions
    pub fn total_gas(transactions: &[TransactionCandidate]) -> u64 {
        transactions.iter().map(|tx| tx.gas_limit).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::U256;

    #[test]
    fn test_signature_validator() {
        let validator = SignatureValidator;

        let valid_tx = TransactionCandidate {
            transaction: vec![],
            from: [0u8; 20],
            nonce: 0,
            gas_price: U256::zero(),
            gas_limit: 21000,
            signature_valid: true,
        };

        assert!(validator.validate(&valid_tx).is_ok());

        let invalid_tx = TransactionCandidate {
            signature_valid: false,
            ..valid_tx
        };

        assert!(validator.validate(&invalid_tx).is_err());
    }

    #[test]
    fn test_gas_validator() {
        let validator = GasValidator::new(30_000_000, U256::from(10));

        let valid_tx = TransactionCandidate {
            transaction: vec![],
            from: [0u8; 20],
            nonce: 0,
            gas_price: U256::from(100),
            gas_limit: 21000,
            signature_valid: true,
        };

        assert!(validator.validate(&valid_tx).is_ok());

        // Gas limit too high
        let invalid_tx = TransactionCandidate {
            gas_limit: 50_000_000,
            ..valid_tx
        };

        assert!(validator.validate(&invalid_tx).is_err());
    }

    #[test]
    fn test_composite_validator() {
        let validator = CompositeValidator::new()
            .add_validator(SignatureValidator)
            .add_validator(GasValidator::new(30_000_000, U256::from(10)));

        let txs = vec![TransactionCandidate {
            transaction: vec![],
            from: [0u8; 20],
            nonce: 0,
            gas_price: U256::from(100),
            gas_limit: 21000,
            signature_valid: true,
        }];

        assert!(validator.validate_batch(&txs).is_ok());
    }

    #[test]
    fn test_check_duplicates() {
        let txs = vec![vec![1, 2, 3], vec![4, 5, 6]];
        assert!(batch::check_duplicates(&txs).is_ok());

        let dup_txs = vec![vec![1, 2, 3], vec![1, 2, 3]];
        assert!(batch::check_duplicates(&dup_txs).is_err());
    }

    #[test]
    fn test_total_gas() {
        let txs = vec![
            TransactionCandidate {
                transaction: vec![],
                from: [0u8; 20],
                nonce: 0,
                gas_price: U256::zero(),
                gas_limit: 21000,
                signature_valid: true,
            },
            TransactionCandidate {
                transaction: vec![],
                from: [1u8; 20],
                nonce: 0,
                gas_price: U256::zero(),
                gas_limit: 50000,
                signature_valid: true,
            },
        ];

        assert_eq!(batch::total_gas(&txs), 71000);
    }
}
