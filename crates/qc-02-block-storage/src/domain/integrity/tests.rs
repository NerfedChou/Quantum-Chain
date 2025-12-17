//! # Integrity Tests

use super::*;

#[test]
fn test_error_display() {
    let err = StorageError::ParentNotFound {
        parent_hash: [0xAB; 32],
    };
    let msg = format!("{}", err);
    assert!(msg.contains("INVARIANT-1"));
    assert!(msg.contains("Parent block not found"));
}

#[test]
fn test_kv_error_conversion() {
    let kv_err = KVStoreError::IOError {
        message: "disk failure".to_string(),
    };
    let storage_err: StorageError = kv_err.into();

    match storage_err {
        StorageError::DatabaseError { message } => {
            assert!(message.contains("disk failure"));
        }
        _ => panic!("Expected DatabaseError"),
    }
}
