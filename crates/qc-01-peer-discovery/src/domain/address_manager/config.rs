//! Address Manager configuration.

/// Configuration for the address manager
#[derive(Debug, Clone)]
pub struct AddressManagerConfig {
    /// Number of buckets in the New table
    pub new_bucket_count: usize,
    /// Number of buckets in the Tried table
    pub tried_bucket_count: usize,
    /// Maximum entries per bucket
    pub bucket_size: usize,
    /// Maximum entries from same /16 subnet per bucket
    pub max_per_subnet_per_bucket: usize,
    /// Maximum entries from same /16 subnet across all buckets
    pub max_per_subnet_total: usize,
}

impl Default for AddressManagerConfig {
    fn default() -> Self {
        Self {
            new_bucket_count: 1024,
            tried_bucket_count: 256,
            bucket_size: 64,
            max_per_subnet_per_bucket: 2,
            max_per_subnet_total: 64,
        }
    }
}

impl AddressManagerConfig {
    /// Testing config with smaller tables
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            new_bucket_count: 16,
            tried_bucket_count: 8,
            bucket_size: 4,
            max_per_subnet_per_bucket: 2,
            max_per_subnet_total: 8,
        }
    }
}
