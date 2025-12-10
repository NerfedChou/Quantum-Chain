//! # Header Sync
//!
//! Header chain synchronization algorithms.
//!
//! Reference: System.md Line 627

use crate::domain::{BlockHeader, HeaderChain, LightClientError};

/// Validate a batch of headers for chain continuity.
///
/// Reference: System.md Line 627
///
/// # Checks
/// 1. Parent hash continuity
/// 2. Height increment
/// 3. Timestamp progression
pub fn validate_header_batch(headers: &[BlockHeader]) -> Result<(), LightClientError> {
    if headers.is_empty() {
        return Ok(());
    }

    for window in headers.windows(2) {
        let prev = &window[0];
        let curr = &window[1];

        // 1. Parent hash continuity
        if curr.parent_hash != prev.hash {
            return Err(LightClientError::InvalidHeaderChain(format!(
                "Broken chain at height {}: expected parent {:?}, got {:?}",
                curr.height, prev.hash, curr.parent_hash
            )));
        }

        // 2. Height increment
        if curr.height != prev.height + 1 {
            return Err(LightClientError::InvalidHeaderChain(format!(
                "Height gap at {}: expected {}, got {}",
                curr.height,
                prev.height + 1,
                curr.height
            )));
        }

        // 3. Timestamp progression
        if curr.timestamp <= prev.timestamp {
            return Err(LightClientError::InvalidHeaderChain(format!(
                "Timestamp not increasing at height {}",
                curr.height
            )));
        }
    }

    Ok(())
}

/// Append a batch of headers to a chain.
pub fn append_headers_batch(
    chain: &mut HeaderChain,
    headers: &[BlockHeader],
) -> Result<u64, LightClientError> {
    // First validate the batch internally
    validate_header_batch(headers)?;

    let mut count = 0;
    for header in headers {
        chain.append(header.clone())?;
        count += 1;
    }

    Ok(count)
}

/// Find the common ancestor height between local chain and remote headers.
pub fn find_common_ancestor(
    local_chain: &HeaderChain,
    remote_headers: &[BlockHeader],
) -> Option<u64> {
    for header in remote_headers.iter().rev() {
        if local_chain.get_header(&header.hash).is_some() {
            return Some(header.height);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hash(n: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = n;
        h
    }

    fn create_valid_chain(count: usize) -> Vec<BlockHeader> {
        let mut headers = Vec::with_capacity(count);
        let genesis = BlockHeader::genesis(make_hash(0), 1000, make_hash(100));
        headers.push(genesis);

        for i in 1..count {
            let prev = &headers[i - 1];
            let header = BlockHeader::new(
                make_hash(i as u8),
                prev.hash,
                i as u64,
                prev.timestamp + 600, // 10 min blocks
                make_hash((i + 100) as u8),
            );
            headers.push(header);
        }

        headers
    }

    #[test]
    fn test_validate_header_batch_empty() {
        assert!(validate_header_batch(&[]).is_ok());
    }

    #[test]
    fn test_validate_header_batch_valid() {
        let headers = create_valid_chain(5);
        assert!(validate_header_batch(&headers).is_ok());
    }

    #[test]
    fn test_validate_header_batch_broken_chain() {
        let mut headers = create_valid_chain(3);
        headers[2].parent_hash = make_hash(99); // Break the chain
        assert!(validate_header_batch(&headers).is_err());
    }

    #[test]
    fn test_validate_header_batch_height_gap() {
        let mut headers = create_valid_chain(3);
        headers[2].height = 10; // Skip heights
        assert!(validate_header_batch(&headers).is_err());
    }

    #[test]
    fn test_validate_header_batch_timestamp_not_increasing() {
        let mut headers = create_valid_chain(3);
        headers[2].timestamp = 100; // Earlier than prev
        assert!(validate_header_batch(&headers).is_err());
    }

    #[test]
    fn test_append_headers_batch() {
        let headers = create_valid_chain(5);
        let genesis = headers[0].clone();
        let mut chain = HeaderChain::new(genesis);

        let count = append_headers_batch(&mut chain, &headers[1..]).unwrap();
        assert_eq!(count, 4);
        assert_eq!(chain.height(), 4);
    }

    #[test]
    fn test_find_common_ancestor() {
        let headers = create_valid_chain(5);
        let genesis = headers[0].clone();
        let chain = HeaderChain::new(genesis);

        // Only genesis exists in chain
        let common = find_common_ancestor(&chain, &headers);
        assert_eq!(common, Some(0));
    }
}
