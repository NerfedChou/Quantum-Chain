//! OpenCL compute backend
//!
//! Portable GPU acceleration that works on:
//! - NVIDIA GPUs
//! - AMD GPUs  
//! - Intel GPUs
//! - Apple GPUs (via OpenCL 1.2)
//!
//! NOTE: OpenCL Kernel objects contain raw pointers and are not thread-safe.
//! We wrap them in a Mutex to ensure safe concurrent access.

use crate::{Backend, ComputeEngine, ComputeError, DeviceInfo};
use primitive_types::U256;
use std::sync::Mutex;

/// OpenCL SHA256 kernel source
const SHA256_KERNEL: &str = r"
// SHA256 constants
__constant uint K[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

#define ROTR(x, n) (((x) >> (n)) | ((x) << (32 - (n))))
#define CH(x, y, z) (((x) & (y)) ^ (~(x) & (z)))
#define MAJ(x, y, z) (((x) & (y)) ^ ((x) & (z)) ^ ((y) & (z)))
#define EP0(x) (ROTR(x, 2) ^ ROTR(x, 13) ^ ROTR(x, 22))
#define EP1(x) (ROTR(x, 6) ^ ROTR(x, 11) ^ ROTR(x, 25))
#define SIG0(x) (ROTR(x, 7) ^ ROTR(x, 18) ^ ((x) >> 3))
#define SIG1(x) (ROTR(x, 17) ^ ROTR(x, 19) ^ ((x) >> 10))

void sha256_transform(__private uint* state, __private const uchar* data) {
    uint a, b, c, d, e, f, g, h, t1, t2, m[64];
    
    for (int i = 0; i < 16; i++) {
        m[i] = (data[i * 4] << 24) | (data[i * 4 + 1] << 16) |
               (data[i * 4 + 2] << 8) | (data[i * 4 + 3]);
    }
    for (int i = 16; i < 64; i++) {
        m[i] = SIG1(m[i - 2]) + m[i - 7] + SIG0(m[i - 15]) + m[i - 16];
    }

    a = state[0]; b = state[1]; c = state[2]; d = state[3];
    e = state[4]; f = state[5]; g = state[6]; h = state[7];

    for (int i = 0; i < 64; i++) {
        t1 = h + EP1(e) + CH(e, f, g) + K[i] + m[i];
        t2 = EP0(a) + MAJ(a, b, c);
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }

    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

__kernel void pow_mine(
    __global const uchar* header_template,
    const uint header_len,
    __global const uchar* target,
    const ulong nonce_start,
    __global ulong* result_nonce,
    __global uchar* result_hash,
    __global int* found
) {
    ulong gid = get_global_id(0);
    ulong nonce = nonce_start + gid;
    
    // Early exit if already found
    if (*found) return;
    
    // Prepare padded message (header + nonce)
    uchar msg[128];
    for (int i = 0; i < header_len; i++) {
        msg[i] = header_template[i];
    }
    
    // Add nonce (little-endian)
    for (int i = 0; i < 8; i++) {
        msg[header_len + i] = (nonce >> (i * 8)) & 0xFF;
    }
    
    uint total_len = header_len + 8;
    
    // SHA256 padding
    msg[total_len] = 0x80;
    for (int i = total_len + 1; i < 56; i++) {
        msg[i] = 0;
    }
    
    // Length in bits (big-endian)
    ulong bit_len = total_len * 8;
    for (int i = 0; i < 8; i++) {
        msg[63 - i] = (bit_len >> (i * 8)) & 0xFF;
    }
    
    // First SHA256
    uint state[8] = {
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    };
    sha256_transform(state, msg);
    
    // Prepare for second SHA256
    uchar hash1[64];
    for (int i = 0; i < 8; i++) {
        hash1[i * 4] = (state[i] >> 24) & 0xFF;
        hash1[i * 4 + 1] = (state[i] >> 16) & 0xFF;
        hash1[i * 4 + 2] = (state[i] >> 8) & 0xFF;
        hash1[i * 4 + 3] = state[i] & 0xFF;
    }
    hash1[32] = 0x80;
    for (int i = 33; i < 56; i++) hash1[i] = 0;
    hash1[62] = 0x01; // 256 bits = 0x100
    hash1[63] = 0x00;
    
    // Second SHA256
    uint state2[8] = {
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    };
    sha256_transform(state2, hash1);
    
    // Compare with target (big-endian comparison)
    bool below_target = false;
    for (int i = 0; i < 8; i++) {
        uint target_word = (target[i * 4] << 24) | (target[i * 4 + 1] << 16) |
                          (target[i * 4 + 2] << 8) | target[i * 4 + 3];
        if (state2[i] < target_word) {
            below_target = true;
            break;
        } else if (state2[i] > target_word) {
            break;
        }
    }
    
    if (below_target) {
        int old = atomic_cmpxchg(found, 0, 1);
        if (old == 0) {
            *result_nonce = nonce;
            for (int i = 0; i < 8; i++) {
                result_hash[i * 4] = (state2[i] >> 24) & 0xFF;
                result_hash[i * 4 + 1] = (state2[i] >> 16) & 0xFF;
                result_hash[i * 4 + 2] = (state2[i] >> 8) & 0xFF;
                result_hash[i * 4 + 3] = state2[i] & 0xFF;
            }
        }
    }
}
";

/// OpenCL-based compute engine
///
/// The kernel is wrapped in a Mutex because ocl::Kernel contains raw pointers
/// that are not Sync. This ensures thread-safe access.
pub struct OpenCLEngine {
    device_info: DeviceInfo,
    context: ocl::Context,
    queue: ocl::Queue,
    /// Kernel wrapped in Mutex for thread safety (ocl::Kernel is not Sync)
    pow_kernel: Mutex<ocl::Kernel>,
}

impl OpenCLEngine {
    pub fn new() -> Result<Self, ComputeError> {
        // Find the best GPU
        // Use ocl::core::get_platform_ids() directly - it returns Result instead of panicking
        let platform_ids = ocl::core::get_platform_ids().map_err(|e| {
            ComputeError::InitializationFailed(format!(
                "Failed to get OpenCL platforms: {}. Is OpenCL installed?",
                e
            ))
        })?;

        let platform_id = platform_ids.first().cloned().ok_or_else(|| {
            ComputeError::InitializationFailed(
                "No OpenCL platform found. Install GPU drivers with OpenCL support.".to_string(),
            )
        })?;

        // Convert core PlatformId to high-level Platform
        let platform = ocl::Platform::new(platform_id);

        let device = ocl::Device::list(platform, Some(ocl::flags::DeviceType::GPU))
            .map_err(|e| ComputeError::InitializationFailed(e.to_string()))?
            .into_iter()
            .next()
            .or_else(|| {
                // Fallback to any device
                ocl::Device::list(platform, None).ok()?.into_iter().next()
            })
            .ok_or_else(|| {
                ComputeError::InitializationFailed("No OpenCL device found".to_string())
            })?;

        let context = ocl::Context::builder()
            .platform(platform)
            .devices(device)
            .build()
            .map_err(|e| ComputeError::InitializationFailed(e.to_string()))?;

        let queue = ocl::Queue::new(&context, device, None)
            .map_err(|e| ComputeError::InitializationFailed(e.to_string()))?;

        // Build the program
        let program = ocl::Program::builder()
            .src(SHA256_KERNEL)
            .devices(device)
            .build(&context)
            .map_err(|e| ComputeError::InitializationFailed(e.to_string()))?;

        // Create kernel with argument placeholders (ocl requires args declared at build time)
        let pow_kernel = ocl::Kernel::builder()
            .program(&program)
            .name("pow_mine")
            .queue(queue.clone())
            .arg(None::<&ocl::Buffer<u8>>)   // 0: header_template
            .arg(0u32)                        // 1: header_len
            .arg(None::<&ocl::Buffer<u8>>)   // 2: target
            .arg(0u64)                        // 3: nonce_start
            .arg(None::<&ocl::Buffer<u64>>)  // 4: result_nonce
            .arg(None::<&ocl::Buffer<u8>>)   // 5: result_hash
            .arg(None::<&ocl::Buffer<i32>>)  // 6: found
            .build()
            .map_err(|e| ComputeError::InitializationFailed(e.to_string()))?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        // Use info() method for device properties in ocl crate
        let compute_units = device
            .info(ocl::core::DeviceInfo::MaxComputeUnits)
            .ok()
            .and_then(|v| match v {
                ocl::core::DeviceInfoResult::MaxComputeUnits(n) => Some(n),
                _ => None,
            })
            .unwrap_or(1);
        let memory = device
            .info(ocl::core::DeviceInfo::GlobalMemSize)
            .ok()
            .and_then(|v| match v {
                ocl::core::DeviceInfoResult::GlobalMemSize(n) => Some(n),
                _ => None,
            })
            .unwrap_or(0);
        let supports_f64 = device.info(ocl::core::DeviceInfo::DoubleFpConfig).is_ok();

        Ok(Self {
            device_info: DeviceInfo {
                name: device_name,
                backend: Backend::OpenCL,
                compute_units,
                memory_bytes: memory,
                supports_f64,
            },
            context,
            queue,
            pow_kernel: Mutex::new(pow_kernel),
        })
    }

    /// Get reference to the OpenCL context.
    ///
    /// Useful for creating additional buffers or programs.
    pub fn context(&self) -> &ocl::Context {
        &self.context
    }
}

#[async_trait::async_trait]
impl ComputeEngine for OpenCLEngine {
    fn backend(&self) -> Backend {
        Backend::OpenCL
    }

    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    async fn batch_sha256(&self, inputs: &[Vec<u8>]) -> Result<Vec<[u8; 32]>, ComputeError> {
        // For batch hashing, CPU is often faster due to data transfer overhead
        // Use Rayon as fallback
        use rayon::prelude::*;
        use sha2::{Digest, Sha256};

        let results: Vec<[u8; 32]> = inputs
            .par_iter()
            .map(|input| {
                let result = Sha256::digest(input);
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
        // Allocate buffers
        let header_buf = ocl::Buffer::builder()
            .queue(self.queue.clone())
            .flags(ocl::flags::MemFlags::new().read_only().copy_host_ptr())
            .len(header_template.len())
            .copy_host_slice(header_template)
            .build()
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        let mut target_bytes = [0u8; 32];
        target.to_big_endian(&mut target_bytes);

        let target_buf = ocl::Buffer::builder()
            .queue(self.queue.clone())
            .flags(ocl::flags::MemFlags::new().read_only().copy_host_ptr())
            .len(32)
            .copy_host_slice(&target_bytes)
            .build()
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        let result_nonce_buf = ocl::Buffer::<u64>::builder()
            .queue(self.queue.clone())
            .flags(ocl::flags::MemFlags::new().write_only())
            .len(1)
            .build()
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        let result_hash_buf = ocl::Buffer::<u8>::builder()
            .queue(self.queue.clone())
            .flags(ocl::flags::MemFlags::new().write_only())
            .len(32)
            .build()
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        let found_buf = ocl::Buffer::<i32>::builder()
            .queue(self.queue.clone())
            .flags(ocl::flags::MemFlags::new().read_write().copy_host_ptr())
            .len(1)
            .copy_host_slice(&[0i32])
            .build()
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        // Set kernel arguments
        let kernel = self
            .pow_kernel
            .lock()
            .map_err(|e| ComputeError::TaskFailed(format!("Kernel lock poisoned: {}", e)))?;

        kernel
            .set_arg(0, &header_buf)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(1, header_template.len() as u32)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(2, &target_buf)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(3, nonce_start)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(4, &result_nonce_buf)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(5, &result_hash_buf)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
        kernel
            .set_arg(6, &found_buf)
            .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

        // Execute in batches
        let batch_size = 1 << 20; // 1M work items per batch
        let mut current_nonce = nonce_start;
        let end_nonce = nonce_start + nonce_count;

        while current_nonce < end_nonce {
            let work_size = std::cmp::min(batch_size, end_nonce - current_nonce) as usize;

            // SAFETY: OpenCL kernel calls require unsafe. Arguments are validated,
            // and work size is bounded by batch_size and nonce_count.
            unsafe {
                kernel
                    .set_arg(3, current_nonce)
                    .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

                kernel
                    .cmd()
                    .global_work_size(work_size)
                    .enq()
                    .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
            }

            self.queue
                .finish()
                .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

            // Check if found - use Vec for buffer reads (ocl requires slices, not arrays)
            let mut found = vec![0i32; 1];
            found_buf
                .read(&mut found)
                .enq()
                .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

            if found[0] != 0 {
                let mut nonce = vec![0u64; 1];
                let mut hash = vec![0u8; 32];

                result_nonce_buf
                    .read(&mut nonce)
                    .enq()
                    .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;
                result_hash_buf
                    .read(&mut hash)
                    .enq()
                    .map_err(|e| ComputeError::TaskFailed(e.to_string()))?;

                let mut hash_array = [0u8; 32];
                hash_array.copy_from_slice(&hash);
                return Ok(Some((nonce[0], hash_array)));
            }

            current_nonce += batch_size;
        }

        Ok(None)
    }

    async fn batch_verify_ecdsa(
        &self,
        messages: &[[u8; 32]],
        signatures: &[[u8; 65]],
        public_keys: &[[u8; 33]],
    ) -> Result<Vec<bool>, ComputeError> {
        // ECDSA verification is complex on GPU, use CPU fallback
        use k256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
        use rayon::prelude::*;

        if messages.len() != signatures.len() || messages.len() != public_keys.len() {
            return Err(ComputeError::InvalidInput(
                "Mismatched array lengths".to_string(),
            ));
        }

        let results: Vec<bool> = (0..messages.len())
            .into_par_iter()
            .map(|i| {
                let pubkey = match VerifyingKey::from_sec1_bytes(&public_keys[i]) {
                    Ok(pk) => pk,
                    Err(_) => return false,
                };

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
