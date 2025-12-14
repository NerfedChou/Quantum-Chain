use super::Hash;
use sha3::{Digest, Keccak256};

// =============================================================================
// RLP ENCODING HELPERS
// =============================================================================

/// RLP-encode a byte slice.
pub fn rlp_encode_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        vec![data[0]]
    } else if data.len() < 56 {
        let mut result = vec![0x80 + data.len() as u8];
        result.extend_from_slice(data);
        result
    } else {
        let len_bytes = encode_length(data.len());
        let mut result = vec![0xb7 + len_bytes.len() as u8];
        result.extend_from_slice(&len_bytes);
        result.extend_from_slice(data);
        result
    }
}

/// RLP-encode two items as a list.
pub fn rlp_encode_two_items(a: &[u8], b: &[u8]) -> Vec<u8> {
    let encoded_a = rlp_encode_bytes(a);
    let encoded_b = rlp_encode_bytes(b);
    let total_len = encoded_a.len() + encoded_b.len();

    let mut result = Vec::with_capacity(total_len + 9);
    if total_len < 56 {
        result.push(0xc0 + total_len as u8);
    } else {
        let len_bytes = encode_length(total_len);
        result.push(0xf7 + len_bytes.len() as u8);
        result.extend_from_slice(&len_bytes);
    }
    result.extend(encoded_a);
    result.extend(encoded_b);
    result
}

/// RLP-encode multiple items as a list.
pub fn rlp_encode_list_items(items: &[Vec<u8>]) -> Vec<u8> {
    let encoded_items: Vec<Vec<u8>> = items.iter().map(|i| rlp_encode_bytes(i)).collect();
    let total_len: usize = encoded_items.iter().map(|e| e.len()).sum();

    let mut result = Vec::with_capacity(total_len + 9);
    if total_len < 56 {
        result.push(0xc0 + total_len as u8);
    } else {
        let len_bytes = encode_length(total_len);
        result.push(0xf7 + len_bytes.len() as u8);
        result.extend_from_slice(&len_bytes);
    }
    for encoded in encoded_items {
        result.extend(encoded);
    }
    result
}

/// Encode a length as minimal big-endian bytes.
fn encode_length(len: usize) -> Vec<u8> {
    let bytes = len.to_be_bytes();
    let start = bytes
        .iter()
        .position(|&b| b != 0)
        .unwrap_or(bytes.len() - 1);
    bytes[start..].to_vec()
}

/// Compute Keccak256 hash.
pub fn keccak256(data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    hasher.finalize().into()
}
