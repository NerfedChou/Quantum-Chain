use crate::ports::outbound::ChecksumProvider;

/// Default checksum provider using crc32fast.
///
/// Implements CRC32C checksums for INVARIANT-3 (Data Integrity).
#[derive(Default)]
pub struct DefaultChecksumProvider;

impl ChecksumProvider for DefaultChecksumProvider {
    fn compute_crc32c(&self, data: &[u8]) -> u32 {
        crc32fast::hash(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_provider() {
        let provider = DefaultChecksumProvider;

        let data = b"hello world";
        let checksum = provider.compute_crc32c(data);

        assert!(provider.verify_crc32c(data, checksum));
        assert!(!provider.verify_crc32c(data, checksum + 1));
    }
}
