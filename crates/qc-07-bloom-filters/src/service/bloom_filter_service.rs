//! Bloom Filter Service
//!
//! Reference: SPEC-07 Section 3 - Service Layer
//!
//! Orchestrates domain logic and coordinates with external dependencies.

use async_trait::async_trait;
use shared_types::{Address, Hash, SignedTransaction};
use std::sync::Arc;

use crate::domain::{BlockFilter, BloomConfig, BloomFilter};
use crate::error::FilterError;
use crate::ports::{
    BloomFilterApi, MatchResult, MatchedField, TransactionDataProvider, TransactionReceipt,
};

/// Bloom Filter Service implementation
///
/// Implements the `BloomFilterApi` port using injected dependencies.
pub struct BloomFilterService<T: TransactionDataProvider> {
    /// Transaction data provider (driven port)
    tx_provider: Arc<T>,
    /// Default configuration
    default_config: BloomConfig,
}

impl<T: TransactionDataProvider> BloomFilterService<T> {
    /// Create a new service with the given transaction provider
    pub fn new(tx_provider: Arc<T>) -> Self {
        Self {
            tx_provider,
            default_config: BloomConfig::default(),
        }
    }

    /// Create with custom default configuration
    pub fn with_config(tx_provider: Arc<T>, config: BloomConfig) -> Self {
        Self {
            tx_provider,
            default_config: config,
        }
    }

    /// Add privacy noise to a filter
    ///
    /// Reference: System.md Subsystem 7 - Privacy Defenses
    fn add_privacy_noise(&self, filter: &mut BloomFilter, config: &BloomConfig) {
        if config.privacy_noise_percent <= 0.0 {
            return;
        }

        // Calculate how many noise elements to add based on the number of real elements
        // This adds additional fake elements proportional to the real element count
        let n = filter.elements_inserted();
        let noise_elements = ((n as f64) * (config.privacy_noise_percent / 100.0)).ceil() as usize;

        // Ensure at least 1 noise element if noise is enabled and we have elements
        let noise_elements = if noise_elements == 0 && n > 0 {
            1
        } else {
            noise_elements
        };

        // Use deterministic "random" noise based on tweak
        // In production, use proper randomness
        let tweak = filter.tweak();
        for i in 0..noise_elements {
            let fake_element = format!("noise_{}_{}", tweak, i);
            filter.insert(fake_element.as_bytes());
        }
    }

    /// Compute contract creation address
    ///
    /// Address = keccak256(rlp([sender, nonce]))[12:]
    fn compute_contract_address(&self, sender: &Address, nonce: u64) -> Address {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(sender);
        hasher.update(nonce.to_be_bytes());
        let hash = hasher.finalize();

        let mut addr = [0u8; 20];
        addr.copy_from_slice(&hash[12..32]);
        addr
    }
}

#[async_trait]
impl<T: TransactionDataProvider + 'static> BloomFilterApi for BloomFilterService<T> {
    fn create_filter(
        &self,
        addresses: &[Address],
        config: &BloomConfig,
    ) -> Result<BloomFilter, FilterError> {
        // Validate config
        config.validate()?;

        // Check address count
        if addresses.len() > config.max_elements {
            return Err(FilterError::TooManyAddresses {
                count: addresses.len(),
                max: config.max_elements,
            });
        }

        // Create filter with optimal parameters
        let mut filter = BloomFilter::new_with_fpr(addresses.len().max(1), config.target_fpr);

        // Insert all addresses
        for addr in addresses {
            filter.insert(addr);
        }

        // Add privacy noise
        self.add_privacy_noise(&mut filter, config);

        // Check size limit
        if filter.size_bits() > config.max_size_bits {
            return Err(FilterError::FilterTooLarge {
                size: filter.size_bits(),
                max: config.max_size_bits,
            });
        }

        Ok(filter)
    }

    fn matches(
        &self,
        filter: &BloomFilter,
        transaction: &SignedTransaction,
        receipt: Option<&TransactionReceipt>,
    ) -> MatchResult {
        // 1. Check sender address
        if filter.contains(&transaction.from) {
            return MatchResult {
                matches: true,
                matched_field: Some(MatchedField::Sender),
            };
        }

        // 2. Check recipient address
        if let Some(to) = &transaction.to {
            if filter.contains(to) {
                return MatchResult {
                    matches: true,
                    matched_field: Some(MatchedField::Recipient),
                };
            }
        } else {
            // 3. Contract creation - compute address
            let contract_addr = self.compute_contract_address(&transaction.from, transaction.nonce);
            if filter.contains(&contract_addr) {
                return MatchResult {
                    matches: true,
                    matched_field: Some(MatchedField::ContractCreation),
                };
            }
        }

        // 4. Check log addresses
        if let Some(receipt) = receipt {
            for (i, log) in receipt.logs.iter().enumerate() {
                if filter.contains(&log.address) {
                    return MatchResult {
                        matches: true,
                        matched_field: Some(MatchedField::LogAddress(i)),
                    };
                }
            }
        }

        MatchResult {
            matches: false,
            matched_field: None,
        }
    }

    async fn get_filtered_transactions(
        &self,
        block_height: u64,
        filter: &BloomFilter,
    ) -> Result<Vec<SignedTransaction>, FilterError> {
        // Get all transactions for the block
        let transactions = self.tx_provider.get_transactions(block_height).await?;

        // Filter by matching addresses
        let filtered: Vec<SignedTransaction> = transactions
            .into_iter()
            .filter(|tx| {
                let result = self.matches(filter, tx, None);
                result.matches
            })
            .collect();

        Ok(filtered)
    }

    fn create_block_filter(
        &self,
        block_hash: Hash,
        block_height: u64,
        _tx_hashes: &[Hash],
        addresses: &[Address],
    ) -> Result<BlockFilter, FilterError> {
        let config = &self.default_config;

        Ok(BlockFilter::new(
            block_hash,
            block_height,
            addresses,
            config,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DataError;
    use shared_types::U256;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    /// Mock transaction provider for testing
    struct MockTxProvider {
        transactions: RwLock<HashMap<u64, Vec<SignedTransaction>>>,
    }

    impl MockTxProvider {
        fn new() -> Self {
            Self {
                transactions: RwLock::new(HashMap::new()),
            }
        }

        async fn add_transaction(&self, height: u64, tx: SignedTransaction) {
            let mut txs = self.transactions.write().await;
            txs.entry(height).or_default().push(tx);
        }
    }

    #[async_trait]
    impl TransactionDataProvider for MockTxProvider {
        async fn get_transaction_hashes(&self, block_height: u64) -> Result<Vec<Hash>, DataError> {
            let txs = self.transactions.read().await;
            match txs.get(&block_height) {
                Some(transactions) => Ok(transactions.iter().map(|tx| tx.hash()).collect()),
                None => Err(DataError::BlockNotFound {
                    height: block_height,
                }),
            }
        }

        async fn get_transactions(
            &self,
            block_height: u64,
        ) -> Result<Vec<SignedTransaction>, DataError> {
            let txs = self.transactions.read().await;
            match txs.get(&block_height) {
                Some(transactions) => Ok(transactions.clone()),
                None => Err(DataError::BlockNotFound {
                    height: block_height,
                }),
            }
        }

        async fn get_transaction_addresses(
            &self,
            block_height: u64,
        ) -> Result<Vec<crate::ports::TransactionAddresses>, DataError> {
            let txs = self.transactions.read().await;
            match txs.get(&block_height) {
                Some(transactions) => Ok(transactions
                    .iter()
                    .map(|tx| crate::ports::TransactionAddresses {
                        tx_hash: tx.hash(),
                        sender: tx.from,
                        recipient: tx.to,
                        created_contract: None,
                        log_addresses: vec![],
                    })
                    .collect()),
                None => Err(DataError::BlockNotFound {
                    height: block_height,
                }),
            }
        }
    }

    fn create_test_tx(from: Address, to: Option<Address>) -> SignedTransaction {
        SignedTransaction {
            from,
            to,
            value: U256::from(100),
            nonce: 1,
            gas_price: U256::from(1),
            gas_limit: 21000,
            data: vec![],
            signature: [0u8; 64],
        }
    }

    fn valid_config() -> BloomConfig {
        BloomConfig {
            target_fpr: 0.05, // Valid FPR
            max_elements: 100,
            max_size_bits: 10000,
            rotation_interval: 100,
            privacy_noise_percent: 0.0, // Disable noise for deterministic tests
        }
    }

    #[tokio::test]
    async fn test_create_filter_for_addresses() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let alice = [0xAA; 20];
        let bob = [0xBB; 20];
        let charlie = [0xCC; 20];

        let filter = service
            .create_filter(&[alice, bob, charlie], &valid_config())
            .unwrap();

        assert!(filter.contains(&alice));
        assert!(filter.contains(&bob));
        assert!(filter.contains(&charlie));
    }

    #[tokio::test]
    async fn test_transaction_matches_sender() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let alice = [0xAA; 20];
        let filter = service.create_filter(&[alice], &valid_config()).unwrap();

        // Create transaction FROM alice
        let tx = create_test_tx(alice, Some([0xFF; 20]));

        let result = service.matches(&filter, &tx, None);
        assert!(result.matches);
        assert_eq!(result.matched_field, Some(MatchedField::Sender));
    }

    #[tokio::test]
    async fn test_transaction_matches_recipient() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let bob = [0xBB; 20];
        let filter = service.create_filter(&[bob], &valid_config()).unwrap();

        // Create transaction TO bob
        let tx = create_test_tx([0xFF; 20], Some(bob));

        let result = service.matches(&filter, &tx, None);
        assert!(result.matches);
        assert_eq!(result.matched_field, Some(MatchedField::Recipient));
    }

    #[tokio::test]
    async fn test_transaction_matches_log_address() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let uniswap_router = [0xDE; 20];
        // Use a larger filter to reduce false positives
        let config = BloomConfig {
            target_fpr: 0.05,
            max_elements: 100,
            max_size_bits: 50000, // Larger filter
            rotation_interval: 100,
            privacy_noise_percent: 0.0,
        };
        let filter = service.create_filter(&[uniswap_router], &config).unwrap();

        // Transaction with log from uniswap router
        // Use addresses that won't collide with uniswap_router
        let tx = create_test_tx([0x01; 20], Some([0x02; 20]));
        let receipt = TransactionReceipt {
            tx_hash: tx.hash(),
            logs: vec![crate::ports::LogEntry {
                address: uniswap_router,
                topics: vec![],
                data: vec![],
            }],
        };

        let result = service.matches(&filter, &tx, Some(&receipt));
        assert!(result.matches);
        assert_eq!(result.matched_field, Some(MatchedField::LogAddress(0)));
    }

    #[tokio::test]
    async fn test_get_filtered_transactions() {
        let provider = Arc::new(MockTxProvider::new());

        let alice = [0xAA; 20];
        let bob = [0xBB; 20];
        let charlie = [0xCC; 20];

        // Add transactions: 2 to alice, 1 to bob, 1 to charlie
        provider
            .add_transaction(1, create_test_tx([0x11; 20], Some(alice)))
            .await;
        provider
            .add_transaction(1, create_test_tx([0x22; 20], Some(bob)))
            .await;
        provider
            .add_transaction(1, create_test_tx([0x33; 20], Some(charlie)))
            .await;
        provider
            .add_transaction(1, create_test_tx([0x44; 20], Some(alice)))
            .await;

        let service = BloomFilterService::new(provider);

        // Filter for alice only
        let filter = service.create_filter(&[alice], &valid_config()).unwrap();

        let filtered = service.get_filtered_transactions(1, &filter).await.unwrap();

        // Should get alice's 2 transactions (plus possible false positives)
        assert!(
            filtered.len() >= 2,
            "Should have at least 2 transactions for alice"
        );
        assert!(filtered.iter().any(|tx| tx.to == Some(alice)));
    }

    #[tokio::test]
    async fn test_reject_too_many_addresses() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let config = BloomConfig {
            target_fpr: 0.05,
            max_elements: 10,
            ..Default::default()
        };

        // Try to create filter with too many addresses
        let addresses: Vec<Address> = (0..20).map(|i| [i as u8; 20]).collect();

        let result = service.create_filter(&addresses, &config);
        assert!(matches!(result, Err(FilterError::TooManyAddresses { .. })));
    }

    #[tokio::test]
    async fn test_privacy_noise_added() {
        let provider = Arc::new(MockTxProvider::new());
        let service = BloomFilterService::new(provider);

        let config = BloomConfig {
            target_fpr: 0.05,
            max_elements: 100,
            max_size_bits: 10000,
            rotation_interval: 100,
            privacy_noise_percent: 20.0, // 20% noise
        };

        let addresses: Vec<Address> = (0..10).map(|i| [i as u8; 20]).collect();
        let filter = service.create_filter(&addresses, &config).unwrap();

        // With 10 addresses and 20% noise, we expect 12 elements (10 + 2 noise)
        // elements_inserted should be > 10
        assert!(
            filter.elements_inserted() > 10,
            "Filter should have extra elements from privacy noise: got {} expected > 10",
            filter.elements_inserted()
        );
    }
}
