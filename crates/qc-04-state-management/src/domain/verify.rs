//! # Iterative Proof Verification (Stack Safety)
//!
//! Loop-based state proof verification to prevent stack overflow attacks.
//!
//! ## Threat
//!
//! Malicious user submits StateProof with 10,000 nodes.
//! Recursive verification blows the stack.
//!
//! ## Defense: Nibble-Walking Loop
//!
//! Strictly iterative verification with depth limits.

use super::{
    AccountState, Address, Hash, StateProof, BRANCH_DOMAIN, EXTENSION_DOMAIN, LEAF_DOMAIN,
    MAX_PROOF_DEPTH,
};
use sha3::{Digest, Keccak256};

/// Errors during iterative verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyError {
    /// Proof exceeds maximum depth
    ProofTooDeep { depth: usize, max: usize },
    /// Empty proof
    EmptyProof,
    /// Path mismatch during verification
    PathMismatch,
    /// Invalid node structure
    InvalidNode,
    /// Hash mismatch
    HashMismatch,
    /// Unexpected node type
    UnexpectedNodeType,
}

/// Node type in proof.
#[derive(Debug, Clone)]
pub enum ProofNode {
    Leaf {
        path: Vec<u8>,
        value: Vec<u8>,
    },
    Extension {
        path: Vec<u8>,
        child_hash: Hash,
    },
    Branch {
        children: Box<[Option<Hash>; 16]>,
        value: Option<Vec<u8>>,
    },
}

/// Verify a state proof iteratively (no recursion).
///
/// ## Algorithm: Nibble-Walking Loop
///
/// 1. Check depth limit (MAX_PROOF_DEPTH)
/// 2. Convert address to nibbles
/// 3. Walk proof nodes, updating current hash
/// 4. Match leaf at end
pub fn verify_proof_iterative(
    proof: &StateProof,
    expected_account: Option<&AccountState>,
) -> Result<bool, VerifyError> {
    // Check depth limit (anti-DoS)
    if proof.proof_nodes.len() > MAX_PROOF_DEPTH {
        return Err(VerifyError::ProofTooDeep {
            depth: proof.proof_nodes.len(),
            max: MAX_PROOF_DEPTH,
        });
    }

    if proof.proof_nodes.is_empty() {
        return Err(VerifyError::EmptyProof);
    }

    // Convert address to nibbles for path matching
    let key_nibbles = address_to_nibbles(&proof.address);
    let mut nibble_idx = 0;

    // Start with state root
    let mut current_hash = proof.state_root;

    // Walk proof nodes iteratively (NO RECURSION)
    for (i, node_bytes) in proof.proof_nodes.iter().enumerate() {
        // Decode node (simplified - production would use RLP)
        let node = decode_proof_node(node_bytes)?;

        match node {
            ProofNode::Leaf { path, value } => {
                // Verify path matches remainder of key
                let remaining = &key_nibbles[nibble_idx..];
                if path != remaining {
                    return Err(VerifyError::PathMismatch);
                }

                // Verify node hash with domain separation
                let computed_hash = hash_with_domain(LEAF_DOMAIN, node_bytes);
                if i > 0 && computed_hash != current_hash {
                    return Err(VerifyError::HashMismatch);
                }

                // Leaf found - verify value matches expected account
                if let Some(expected) = expected_account {
                    let expected_bytes = serialize_account(expected);
                    return Ok(value == expected_bytes);
                }
                return Ok(true);
            }

            ProofNode::Extension { path, child_hash } => {
                // Verify path matches
                let path_len = path.len();
                if nibble_idx + path_len > key_nibbles.len() {
                    return Err(VerifyError::PathMismatch);
                }

                let key_segment = &key_nibbles[nibble_idx..nibble_idx + path_len];
                if path != key_segment {
                    return Err(VerifyError::PathMismatch);
                }

                // Verify node hash with domain separation
                let computed_hash = hash_with_domain(EXTENSION_DOMAIN, node_bytes);
                if i > 0 && computed_hash != current_hash {
                    return Err(VerifyError::HashMismatch);
                }

                nibble_idx += path_len;
                current_hash = child_hash;
            }

            ProofNode::Branch { children, value: _ } => {
                if nibble_idx >= key_nibbles.len() {
                    return Err(VerifyError::PathMismatch);
                }

                // Verify node hash with domain separation
                let computed_hash = hash_with_domain(BRANCH_DOMAIN, node_bytes);
                if i > 0 && computed_hash != current_hash {
                    return Err(VerifyError::HashMismatch);
                }

                // Get next nibble and follow child
                let next_nibble = key_nibbles[nibble_idx] as usize;
                nibble_idx += 1;

                match &children[next_nibble] {
                    Some(child) => current_hash = *child,
                    None => return Err(VerifyError::PathMismatch),
                }
            }
        }
    }

    // If we exhausted proof without finding leaf, it's invalid
    Err(VerifyError::InvalidNode)
}

/// Convert address to nibbles.
fn address_to_nibbles(address: &Address) -> Vec<u8> {
    let mut nibbles = Vec::with_capacity(40);
    for byte in address {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0F);
    }
    nibbles
}

/// Hash with domain separation.
fn hash_with_domain(domain: u8, data: &[u8]) -> Hash {
    let mut hasher = Keccak256::new();
    hasher.update([domain]);
    hasher.update(data);
    hasher.finalize().into()
}

/// Decode proof node (simplified).
fn decode_proof_node(data: &[u8]) -> Result<ProofNode, VerifyError> {
    if data.is_empty() {
        return Err(VerifyError::InvalidNode);
    }

    // First byte indicates node type (simplified encoding)
    match data[0] {
        0x00 => {
            // Leaf node: [0x00, path_len, path..., value...]
            if data.len() < 3 {
                return Err(VerifyError::InvalidNode);
            }
            let path_len = data[1] as usize;
            if data.len() < 2 + path_len {
                return Err(VerifyError::InvalidNode);
            }
            let path = data[2..2 + path_len].to_vec();
            let value = data[2 + path_len..].to_vec();
            Ok(ProofNode::Leaf { path, value })
        }
        0x01 => {
            // Extension node: [0x01, path_len, path..., child_hash (32)]
            if data.len() < 35 {
                return Err(VerifyError::InvalidNode);
            }
            let path_len = data[1] as usize;
            if data.len() < 2 + path_len + 32 {
                return Err(VerifyError::InvalidNode);
            }
            let path = data[2..2 + path_len].to_vec();
            let mut child_hash = [0u8; 32];
            child_hash.copy_from_slice(&data[2 + path_len..2 + path_len + 32]);
            Ok(ProofNode::Extension { path, child_hash })
        }
        0x02 => {
            // Branch node (simplified): just children
            let children: Box<[Option<Hash>; 16]> = Box::new([None; 16]);
            Ok(ProofNode::Branch {
                children,
                value: None,
            })
        }
        _ => Err(VerifyError::UnexpectedNodeType),
    }
}

/// Serialize account (simplified).
fn serialize_account(account: &AccountState) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&account.balance.to_le_bytes());
    data.extend_from_slice(&account.nonce.to_le_bytes());
    data.extend_from_slice(&account.code_hash);
    data.extend_from_slice(&account.storage_root);
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_proof(nodes: usize) -> StateProof {
        StateProof {
            address: [0x01; 20],
            account_state: Some(AccountState::new(1000)),
            proof_nodes: (0..nodes).map(|_| vec![0x00, 0, 0]).collect(),
            state_root: [0xAA; 32],
        }
    }

    #[test]
    fn test_proof_too_deep() {
        let proof = make_test_proof(100); // Exceeds MAX_PROOF_DEPTH
        let result = verify_proof_iterative(&proof, None);

        assert!(matches!(result, Err(VerifyError::ProofTooDeep { .. })));
    }

    #[test]
    fn test_empty_proof() {
        let proof = StateProof {
            address: [0x01; 20],
            account_state: None,
            proof_nodes: vec![],
            state_root: [0xAA; 32],
        };

        let result = verify_proof_iterative(&proof, None);
        assert!(matches!(result, Err(VerifyError::EmptyProof)));
    }

    #[test]
    fn test_address_to_nibbles() {
        let addr = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let nibbles = address_to_nibbles(&addr);

        assert_eq!(nibbles.len(), 40);
        assert_eq!(nibbles[0], 0x01);
        assert_eq!(nibbles[1], 0x02);
        assert_eq!(nibbles[2], 0x03);
        assert_eq!(nibbles[3], 0x04);
    }

    #[test]
    fn test_hash_with_domain() {
        let data = b"test";

        let h1 = hash_with_domain(LEAF_DOMAIN, data);
        let h2 = hash_with_domain(EXTENSION_DOMAIN, data);
        let h3 = hash_with_domain(BRANCH_DOMAIN, data);

        // Different domains produce different hashes
        assert_ne!(h1, h2);
        assert_ne!(h1, h3);
        assert_ne!(h2, h3);
    }

    #[test]
    fn test_decode_leaf_node() {
        // [0x00, path_len=2, path..., value...]
        let data = vec![0x00, 2, 0x01, 0x02, 0xAA, 0xBB];
        let node = decode_proof_node(&data).unwrap();

        match node {
            ProofNode::Leaf { path, value } => {
                assert_eq!(path, vec![0x01, 0x02]);
                assert_eq!(value, vec![0xAA, 0xBB]);
            }
            _ => panic!("Expected Leaf"),
        }
    }
}
