//! # Repair Tests

use super::*;

#[test]
fn test_repair_report_new() {
    let report = RepairReport::new();
    assert_eq!(report.blocks_recovered, 0);
    assert_eq!(report.transactions_indexed, 0);
    assert!(report.lowest_height.is_none());
    assert!(report.highest_height.is_none());
}

#[test]
fn test_repair_report_add_block() {
    let mut report = RepairReport::new();

    report.add_block(100, 5);
    assert_eq!(report.blocks_recovered, 1);
    assert_eq!(report.transactions_indexed, 5);
    assert_eq!(report.lowest_height, Some(100));
    assert_eq!(report.highest_height, Some(100));

    report.add_block(50, 3);
    assert_eq!(report.blocks_recovered, 2);
    assert_eq!(report.transactions_indexed, 8);
    assert_eq!(report.lowest_height, Some(50));
    assert_eq!(report.highest_height, Some(100));

    report.add_block(200, 10);
    assert_eq!(report.highest_height, Some(200));
}

#[test]
fn test_repair_report_is_successful() {
    let mut report = RepairReport::new();
    assert!(report.is_successful());

    report.add_block(1, 1);
    assert!(report.is_successful());
}

#[test]
fn test_repair_context_record_block() {
    let mut ctx = RepairContext::new();
    let hash = [1u8; 32];

    ctx.record_block(100, hash, 5);

    assert_eq!(ctx.block_index.get(&100), Some(&hash));
    assert_eq!(ctx.report.blocks_recovered, 1);
}

#[test]
fn test_repair_context_record_transaction() {
    let mut ctx = RepairContext::new();
    let tx_hash = [1u8; 32];
    let block_hash = [2u8; 32];

    ctx.record_transaction(tx_hash, block_hash, 0);

    assert!(ctx.tx_index.contains_key(&tx_hash));
}

#[test]
fn test_repair_error_display() {
    let error = RepairError::new(vec![1, 2, 3], "test error");
    assert_eq!(error.message, "test error");
}
