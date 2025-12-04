//! Web3 JSON-RPC methods per SPEC-16 Section 3.1.

use crate::domain::error::ApiResult;
use crate::domain::types::Bytes;
use sha3::{Digest, Keccak256};

/// Web3 RPC methods handler
pub struct Web3Rpc {
    client_version: String,
}

impl Web3Rpc {
    pub fn new(client_version: String) -> Self {
        Self { client_version }
    }

    /// web3_clientVersion - Returns client version string
    ///
    /// Format: QuantumChain/v{version}/{os}/{runtime}
    pub async fn client_version(&self) -> ApiResult<String> {
        Ok(self.client_version.clone())
    }

    /// web3_sha3 - Returns Keccak-256 hash of data
    ///
    /// Note: This is NOT SHA3-256 (standardized), it's Keccak-256 (pre-standardization).
    /// Ethereum uses Keccak-256 throughout.
    pub async fn sha3(&self, data: Bytes) -> ApiResult<String> {
        let mut hasher = Keccak256::new();
        hasher.update(data.as_slice());
        let result = hasher.finalize();
        Ok(format!("0x{}", hex::encode(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_version() {
        let web3 = Web3Rpc::new("QuantumChain/v0.1.0/linux/rust".to_string());
        let version = web3.client_version().await.unwrap();
        assert!(version.contains("QuantumChain"));
    }

    #[tokio::test]
    async fn test_sha3_empty() {
        let web3 = Web3Rpc::new("test".to_string());
        let result = web3.sha3(Bytes::new()).await.unwrap();
        // Keccak-256 of empty string
        assert_eq!(
            result,
            "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[tokio::test]
    async fn test_sha3_hello() {
        let web3 = Web3Rpc::new("test".to_string());
        let result = web3.sha3(Bytes::from_slice(b"hello")).await.unwrap();
        // Keccak-256 of "hello"
        assert_eq!(
            result,
            "0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        );
    }

    #[tokio::test]
    async fn test_sha3_hex() {
        let web3 = Web3Rpc::new("test".to_string());
        let data = Bytes::from_slice(&hex::decode("68656c6c6f").unwrap()); // "hello" in hex
        let result = web3.sha3(data).await.unwrap();
        // Should be same as "hello"
        assert_eq!(
            result,
            "0x1c8aff950685c2ed4bc3174f3472287b56d9517b9c948127319a09a7a36deac8"
        );
    }
}
