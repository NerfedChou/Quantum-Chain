//! # MMR Tests

use super::*;
use shared_types::Hash;

fn make_leaf(n: u8) -> Hash {
    let mut h = [0u8; 32];
    h[0] = n;
    h
}

#[test]
fn test_mmr_new_empty() {
    let mmr = MmrStore::new();
    assert_eq!(mmr.leaf_count(), 0);
    assert!(mmr.peaks().is_empty());
    assert_eq!(mmr.root(), [0u8; 32]);
}

#[test]
fn test_mmr_append_single() {
    let mut mmr = MmrStore::new();
    let leaf = make_leaf(1);
    let index = mmr.append(leaf);

    assert_eq!(index, 0);
    assert_eq!(mmr.leaf_count(), 1);
    assert_eq!(mmr.peaks().len(), 1);
    assert_eq!(mmr.peaks()[0], leaf);
}

#[test]
fn test_mmr_append_merges_peaks() {
    let mut mmr = MmrStore::new();
    mmr.append(make_leaf(1));
    mmr.append(make_leaf(2));

    assert_eq!(mmr.leaf_count(), 2);
    assert_eq!(mmr.peaks().len(), 1);
}

#[test]
fn test_mmr_append_three_leaves() {
    let mut mmr = MmrStore::new();
    mmr.append(make_leaf(1));
    mmr.append(make_leaf(2));
    mmr.append(make_leaf(3));

    assert_eq!(mmr.leaf_count(), 3);
    assert_eq!(mmr.peaks().len(), 2);
}

#[test]
fn test_mmr_root_changes() {
    let mut mmr = MmrStore::new();
    let root0 = mmr.root();
    mmr.append(make_leaf(1));
    let root1 = mmr.root();
    mmr.append(make_leaf(2));
    let root2 = mmr.root();

    assert_ne!(root0, root1);
    assert_ne!(root1, root2);
}

#[test]
fn test_mmr_proof_generation() {
    let mut mmr = MmrStore::new();
    mmr.append(make_leaf(1));
    mmr.append(make_leaf(2));

    let proof = mmr.get_proof(0).expect("proof");
    assert_eq!(proof.leaf_index, 0);
    assert_eq!(proof.leaf_count, 2);
}

#[test]
fn test_mmr_proof_not_found() {
    let mmr = MmrStore::new();
    let result = mmr.get_proof(0);
    assert!(matches!(result, Err(MmrError::LeafNotFound { .. })));
}

#[test]
fn test_mmr_verify_proof() {
    let mut mmr = MmrStore::new();
    let leaf = make_leaf(1);
    mmr.append(leaf);

    let root = mmr.root();
    let proof = mmr.get_proof(0).expect("proof");

    assert!(MmrStore::verify_proof(&root, &leaf, &proof));
}
