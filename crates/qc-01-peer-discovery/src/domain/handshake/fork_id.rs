//! Fork ID (EIP-2124 inspired).

/// Fork ID for quick network/fork identification
///
/// Compact representation: hash(genesis + fork_hashes) + next_fork
///
/// Reference: EIP-2124
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForkId {
    /// CRC32 hash of genesis + all fork block hashes
    pub hash: u32,
    /// Block number of next expected fork (0 if none)
    pub next: u64,
}

impl ForkId {
    /// Create a new fork ID
    pub fn new(hash: u32, next: u64) -> Self {
        Self { hash, next }
    }

    /// Check if two fork IDs are compatible (EIP-2124 logic).
    ///
    /// # Compatibility Rules (per EIP-2124)
    ///
    /// 1. **Hash mismatch**: If hashes differ → incompatible
    /// 2. **We're stale**: If their `next` fork is in the past → incompatible
    /// 3. **They're stale**: If our `next` fork is in the past → incompatible
    /// 4. **Future fork**: If `next` is in the future for both → compatible
    pub fn is_compatible(&self, other: &ForkId, our_height: u64) -> bool {
        // Rule 1: Hash mismatch = different chain or diverged fork
        if self.hash != other.hash {
            return false;
        }

        // If hashes match, we're on the same chain
        // If either next==0, no more forks expected → compatible
        if self.next == 0 || other.next == 0 {
            return true;
        }

        // Both expect future forks
        if self.next == other.next {
            return true;
        }

        // Different next forks with same hash
        // If their next fork is before our height, they expect us to have it
        if other.next <= our_height {
            return false;
        }

        true
    }

    /// Check if this ForkId indicates a stale node.
    pub fn is_stale(&self, remote_next: u64, our_height: u64) -> bool {
        remote_next != 0 && remote_next <= our_height && remote_next != self.next
    }
}
