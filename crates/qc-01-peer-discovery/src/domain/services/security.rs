//! Security services: Sybil resistance logic.
//!
//! SECURITY-CRITICAL: Contains IP diversity checks.
//! Isolate for security audits.

use crate::domain::{IpAddr, SubnetMask};

/// Check if two IP addresses share the same subnet prefix.
///
/// # Security (Sybil Resistance)
/// Used to enforce INVARIANT-3 (IP Diversity). Prevents a single attacker
/// controlling a subnet from filling all our buckets.
///
/// Compares addresses using the specified prefix length (e.g., /24 for IPv4).
///
/// Reference: SPEC-01 Section 6.1 (Sybil Attack Resistance)
pub fn is_same_subnet(a: &IpAddr, b: &IpAddr, mask: &SubnetMask) -> bool {
    match (a, b) {
        (IpAddr::V4(a_bytes), IpAddr::V4(b_bytes)) => {
            prefix_matches(a_bytes, b_bytes, mask.prefix_length, 4)
        }
        (IpAddr::V6(a_bytes), IpAddr::V6(b_bytes)) => {
            prefix_matches(a_bytes, b_bytes, mask.prefix_length, 16)
        }
        // IPv4 and IPv6 addresses are in disjoint address spaces
        _ => false,
    }
}

/// Compare byte slices up to a prefix length in bits.
///
/// Returns true if the first `prefix_bits` bits of both slices are equal.
fn prefix_matches(a: &[u8], b: &[u8], prefix_bits: u8, max_bytes: usize) -> bool {
    let prefix_bytes = (prefix_bits / 8) as usize;
    let remaining_bits = prefix_bits % 8;

    // Compare full bytes within prefix
    for i in 0..prefix_bytes.min(max_bytes) {
        if a[i] != b[i] {
            return false;
        }
    }

    // Compare partial byte if prefix doesn't align to byte boundary
    if remaining_bits > 0 && prefix_bytes < max_bytes {
        let mask_byte = 0xFF << (8 - remaining_bits);
        return (a[prefix_bytes] & mask_byte) == (b[prefix_bytes] & mask_byte);
    }

    true
}
