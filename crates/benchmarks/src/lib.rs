//! Benchmark utilities for Quantum-Chain subsystems
pub mod utils {
    pub fn generate_random_hash() -> [u8; 32] {
        use rand::Rng;
        let mut hash = [0u8; 32];
        rand::thread_rng().fill(&mut hash);
        hash
    }
}
