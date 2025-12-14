use super::{nibbles::Nibbles, rlp, Hash, EMPTY_TRIE_ROOT};

// =============================================================================
// TRIE NODE: The four node types in MPT
// =============================================================================

/// Node types in the Patricia Merkle Trie.
///
/// Per Ethereum Yellow Paper Appendix D, there are four node types:
/// - Empty (null reference)
/// - Leaf (remaining path + value)
/// - Extension (shared prefix + single child)
/// - Branch (16 children + optional value)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrieNode {
    /// Empty node (null reference, hash = EMPTY_TRIE_ROOT).
    Empty,

    /// Leaf node: stores remaining key path and the value.
    /// RLP: [hex_prefix_encode(path, true), value]
    Leaf {
        /// Remaining path from current position to this leaf.
        path: Nibbles,
        /// RLP-encoded value (account state or storage value).
        value: Vec<u8>,
    },

    /// Extension node: shared prefix optimization.
    /// RLP: [hex_prefix_encode(path, false), child_hash]
    Extension {
        /// Shared prefix path.
        path: Nibbles,
        /// Hash of child node.
        child: Hash,
    },

    /// Branch node: 16-way branch for each nibble value.
    /// RLP: \[child\[0\], ..., child\[15\], value\]
    Branch {
        /// 16 child node hashes (None = empty).
        children: Box<[Option<Hash>; 16]>,
        /// Optional value if a key terminates at this branch.
        value: Option<Vec<u8>>,
    },
}

impl TrieNode {
    /// RLP-encode this node for hashing.
    pub fn rlp_encode(&self) -> Vec<u8> {
        match self {
            TrieNode::Empty => vec![0x80], // RLP empty string

            TrieNode::Leaf { path, value } => {
                let encoded_path = path.encode_hex_prefix(true);
                rlp::rlp_encode_two_items(&encoded_path, value)
            }

            TrieNode::Extension { path, child } => {
                let encoded_path = path.encode_hex_prefix(false);
                rlp::rlp_encode_two_items(&encoded_path, child)
            }

            TrieNode::Branch { children, value } => {
                let mut items: Vec<Vec<u8>> = Vec::with_capacity(17);

                for child in children.iter() {
                    match child {
                        Some(hash) => items.push(hash.to_vec()),
                        None => items.push(vec![0x80]), // Empty
                    }
                }

                match value {
                    Some(v) => items.push(v.clone()),
                    None => items.push(vec![0x80]),
                }

                rlp::rlp_encode_list_items(&items)
            }
        }
    }

    /// Compute Keccak256 hash of RLP-encoded node.
    pub fn hash(&self) -> Hash {
        if matches!(self, TrieNode::Empty) {
            return EMPTY_TRIE_ROOT;
        }
        let encoded = self.rlp_encode();
        rlp::keccak256(&encoded)
    }

    /// Process a single trie node during proof traversal.
    ///
    /// Returns `Some(next_hash)` to continue traversal, or `None` to stop.
    /// This helper reduces nesting depth in proof generation loops.
    pub fn process_for_proof(&self, key: &Nibbles, depth: &mut usize) -> Option<Hash> {
        match self {
            TrieNode::Empty => None,

            TrieNode::Leaf { .. } => None, // Stop at leaf

            TrieNode::Extension { path, child } => {
                let remaining = key.slice(*depth);
                if !remaining.0.starts_with(&path.0) {
                    return None; // Path diverges
                }
                *depth += path.len();
                Some(*child)
            }

            TrieNode::Branch { children, .. } => {
                if *depth >= key.len() {
                    return None; // Boundary reached
                }
                let nibble = key.at(*depth) as usize;
                let child = children[nibble]?;
                *depth += 1;
                Some(child)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_node_hashing() {
        let leaf = TrieNode::Leaf {
            path: Nibbles(vec![1, 2, 3, 4]),
            value: vec![0xAB, 0xCD],
        };

        let hash1 = leaf.hash();
        let hash2 = leaf.hash();

        // Same node should produce same hash
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, EMPTY_TRIE_ROOT);
    }
}
