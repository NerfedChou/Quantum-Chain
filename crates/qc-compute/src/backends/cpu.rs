//! CPU compute backend using Rayon
//!
//! This is the fallback backend that always works. It uses Rayon for
//! parallel execution across CPU cores.

use crate::{Backend, ComputeEngine, ComputeError, DeviceInfo};
use primitive_types::U256;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

/// CPU-based compute engine using Rayon
pub struct CpuEngine {
    device_info: DeviceInfo,
}

impl CpuEngine {
    pub fn new() -> Self {
        let num_cpus = num_cpus::get() as u32;

        Self {
            device_info: DeviceInfo {
                name: format!("CPU ({} cores)", num_cpus),
                backend: Backend::Cpu,
                compute_units: num_cpus,
                memory_bytes: 0, // System memory, not tracked
                supports_f64: true,
            },
        }
    }
}

impl Default for CpuEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ComputeEngine for CpuEngine {
    fn backend(&self) -> Backend {
        Backend::Cpu
    }

    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    async fn batch_sha256(&self, inputs: &[Vec<u8>]) -> Result<Vec<[u8; 32]>, ComputeError> {
        let results: Vec<[u8; 32]> = inputs
            .par_iter()
            .map(|input| {
                let mut hasher = Sha256::new();
                hasher.update(input);
                let result = hasher.finalize();
                let mut output = [0u8; 32];
                output.copy_from_slice(&result);
                output
            })
            .collect();

        Ok(results)
    }

    async fn pow_mine(
        &self,
        header_template: &[u8],
        target: U256,
        nonce_start: u64,
        nonce_count: u64,
    ) -> Result<Option<(u64, [u8; 32])>, ComputeError> {
        use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

        let found = AtomicBool::new(false);
        let result_nonce = AtomicU64::new(0);
        let result_hash: std::sync::Mutex<[u8; 32]> = std::sync::Mutex::new([0u8; 32]);

        let num_threads = self.device_info.compute_units as u64;
        let chunk_size = nonce_count / num_threads;

        (0..num_threads).into_par_iter().for_each(|thread_id| {
            let start = nonce_start + (thread_id * chunk_size);
            let end = if thread_id == num_threads - 1 {
                nonce_start + nonce_count
            } else {
                start + chunk_size
            };

            for nonce in start..end {
                // Early exit if another thread found it
                if found.load(Ordering::Relaxed) {
                    break;
                }

                // Check every 10000 iterations
                if nonce % 10000 == 0 && found.load(Ordering::Relaxed) {
                    break;
                }

                // Build full header with nonce
                let mut full_header = header_template.to_vec();
                full_header.extend_from_slice(&nonce.to_le_bytes());

                // Double SHA256 (Bitcoin-style)
                let hash1 = Sha256::digest(&full_header);
                let hash2 = Sha256::digest(hash1);

                // Convert to U256 for comparison
                let hash_value = U256::from_big_endian(&hash2);

                if hash_value <= target {
                    found.store(true, Ordering::SeqCst);
                    result_nonce.store(nonce, Ordering::SeqCst);
                    let mut hash_lock = result_hash.lock().unwrap();
                    hash_lock.copy_from_slice(&hash2);
                    break;
                }
            }
        });

        if found.load(Ordering::SeqCst) {
            let nonce = result_nonce.load(Ordering::SeqCst);
            let hash = *result_hash.lock().unwrap();
            Ok(Some((nonce, hash)))
        } else {
            Ok(None)
        }
    }

    async fn batch_verify_ecdsa(
        &self,
        messages: &[[u8; 32]],
        signatures: &[[u8; 65]],
        public_keys: &[[u8; 33]],
    ) -> Result<Vec<bool>, ComputeError> {
        if messages.len() != signatures.len() || messages.len() != public_keys.len() {
            return Err(ComputeError::InvalidInput(
                "Mismatched input array lengths".to_string(),
            ));
        }

        // Parallel verification using Rayon
        let results: Vec<bool> = (0..messages.len())
            .into_par_iter()
            .map(|i| {
                // Use k256 for ECDSA verification
                use k256::ecdsa::{signature::Verifier, Signature, VerifyingKey};

                let pubkey = match VerifyingKey::from_sec1_bytes(&public_keys[i]) {
                    Ok(pk) => pk,
                    Err(_) => return false,
                };

                // signatures[i] is 65 bytes: r (32) + s (32) + v (1)
                let sig_bytes = &signatures[i][..64];
                let signature = match Signature::from_slice(sig_bytes) {
                    Ok(s) => s,
                    Err(_) => return false,
                };

                pubkey.verify(&messages[i], &signature).is_ok()
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_batch_sha256() {
        let engine = CpuEngine::new();
        let inputs = vec![b"hello".to_vec(), b"world".to_vec(), b"test".to_vec()];

        let results = engine.batch_sha256(&inputs).await.unwrap();
        assert_eq!(results.len(), 3);

        // Verify first hash
        let expected = Sha256::digest(b"hello");
        assert_eq!(results[0], expected.as_slice());
    }

    #[tokio::test]
    async fn test_pow_mine_easy_target() {
        let engine = CpuEngine::new();

        // Very easy target (high value = easy)
        let target = U256::MAX / 2;
        let header = b"test_header".to_vec();

        let result = engine
            .pow_mine(&header, target, 0, 1_000_000)
            .await
            .unwrap();

        assert!(result.is_some());
        let (nonce, hash) = result.unwrap();

        // Verify the hash is below target
        let hash_value = U256::from_big_endian(&hash);
        assert!(hash_value <= target);
        println!("Found nonce: {}", nonce);
    }
}
