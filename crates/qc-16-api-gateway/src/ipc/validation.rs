//! RLP pre-validation for eth_sendRawTransaction per SPEC-16 Section 4.
//!
//! CRITICAL: Validates transaction RLP structure BEFORE sending to mempool.
//! Rejects garbage at the gate - don't bother subsystems with invalid data.

use crate::domain::error::ApiError;
use crate::domain::types::{Address, Bytes, Hash, U256};
use crate::ipc::requests::SubmitTransactionRequest;
use rlp::{DecoderError, Rlp};
use sha3::{Digest, Keccak256};
use tracing::debug;

/// Maximum allowed transaction size (128 KB)
const MAX_TX_SIZE: usize = 128 * 1024;

/// Minimum transaction size (empty tx ~= 85 bytes)
const MIN_TX_SIZE: usize = 85;

/// Validated transaction info extracted from RLP
#[derive(Debug, Clone)]
pub struct ValidatedTransaction {
    pub hash: Hash,
    pub sender: Address,
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Vec<u8>,
    pub tx_type: TxType,
}

/// Transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxType {
    Legacy,
    AccessList, // EIP-2930
    DynamicFee, // EIP-1559
}

/// Validate raw transaction bytes and extract info.
///
/// This is the SYNTACTIC validation that happens at the API Gateway.
/// Semantic validation (balance, nonce sequence) happens in mempool.
pub fn validate_raw_transaction(raw: &[u8]) -> Result<ValidatedTransaction, ApiError> {
    // Size checks
    if raw.len() < MIN_TX_SIZE {
        return Err(ApiError::invalid_params("Transaction too small"));
    }
    if raw.len() > MAX_TX_SIZE {
        return Err(ApiError::invalid_params(format!(
            "Transaction size {} exceeds limit {}",
            raw.len(),
            MAX_TX_SIZE
        )));
    }

    // Detect transaction type from first byte
    let (tx_type, rlp_data) = detect_tx_type(raw)?;

    // Parse based on type
    let tx_info = match tx_type {
        TxType::Legacy => parse_legacy_tx(rlp_data)?,
        TxType::AccessList => parse_access_list_tx(rlp_data)?,
        TxType::DynamicFee => parse_dynamic_fee_tx(rlp_data)?,
    };

    // Compute transaction hash
    let hash = compute_tx_hash(raw, tx_type);

    // Recover sender from signature
    let sender = recover_sender(&tx_info, tx_type, rlp_data)?;

    debug!(
        hash = %hash,
        sender = %sender,
        nonce = tx_info.nonce,
        gas_price = ?tx_info.gas_price,
        tx_type = ?tx_type,
        "Validated raw transaction"
    );

    Ok(ValidatedTransaction {
        hash,
        sender,
        nonce: tx_info.nonce,
        gas_price: tx_info.gas_price,
        gas_limit: tx_info.gas_limit,
        to: tx_info.to,
        value: tx_info.value,
        data: tx_info.data,
        tx_type,
    })
}

/// Create SubmitTransactionRequest from validated transaction
pub fn create_submit_request(
    raw: Bytes,
    validated: &ValidatedTransaction,
) -> SubmitTransactionRequest {
    SubmitTransactionRequest {
        raw_transaction: raw,
        tx_hash: validated.hash,
        sender: validated.sender,
        nonce: validated.nonce,
        gas_price: validated.gas_price,
        gas_limit: validated.gas_limit,
    }
}

/// Detect transaction type from first byte
fn detect_tx_type(raw: &[u8]) -> Result<(TxType, &[u8]), ApiError> {
    if raw.is_empty() {
        return Err(ApiError::invalid_params("Empty transaction"));
    }

    let first_byte = raw[0];

    // EIP-2718: Typed transactions have first byte < 0x7f
    // Legacy transactions start with RLP list prefix (0xc0-0xff)
    if first_byte < 0x7f {
        match first_byte {
            0x01 => Ok((TxType::AccessList, &raw[1..])),
            0x02 => Ok((TxType::DynamicFee, &raw[1..])),
            _ => Err(ApiError::invalid_params(format!(
                "Unknown transaction type: 0x{:02x}",
                first_byte
            ))),
        }
    } else {
        Ok((TxType::Legacy, raw))
    }
}

/// Parsed transaction fields (common across types)
struct ParsedTxFields {
    nonce: u64,
    gas_price: U256,
    gas_limit: u64,
    to: Option<Address>,
    value: U256,
    data: Vec<u8>,
    v: u64,
    r: [u8; 32],
    s: [u8; 32],
}

/// Parse legacy transaction RLP
fn parse_legacy_tx(data: &[u8]) -> Result<ParsedTxFields, ApiError> {
    let rlp = Rlp::new(data);

    if !rlp.is_list() {
        return Err(ApiError::invalid_params("Transaction must be RLP list"));
    }

    let item_count = rlp.item_count().map_err(|e| rlp_error("item count", e))?;
    if item_count != 9 {
        return Err(ApiError::invalid_params(format!(
            "Legacy transaction must have 9 fields, got {}",
            item_count
        )));
    }

    Ok(ParsedTxFields {
        nonce: decode_u64(&rlp, 0)?,
        gas_price: decode_u256(&rlp, 1)?,
        gas_limit: decode_u64(&rlp, 2)?,
        to: decode_optional_address(&rlp, 3)?,
        value: decode_u256(&rlp, 4)?,
        data: decode_bytes(&rlp, 5)?,
        v: decode_u64(&rlp, 6)?,
        r: decode_bytes32(&rlp, 7)?,
        s: decode_bytes32(&rlp, 8)?,
    })
}

/// Parse EIP-2930 access list transaction
fn parse_access_list_tx(data: &[u8]) -> Result<ParsedTxFields, ApiError> {
    let rlp = Rlp::new(data);

    if !rlp.is_list() {
        return Err(ApiError::invalid_params("Transaction must be RLP list"));
    }

    let item_count = rlp.item_count().map_err(|e| rlp_error("item count", e))?;
    if item_count != 11 {
        return Err(ApiError::invalid_params(format!(
            "EIP-2930 transaction must have 11 fields, got {}",
            item_count
        )));
    }

    // EIP-2930: [chainId, nonce, gasPrice, gasLimit, to, value, data, accessList, yParity, r, s]
    Ok(ParsedTxFields {
        nonce: decode_u64(&rlp, 1)?,
        gas_price: decode_u256(&rlp, 2)?,
        gas_limit: decode_u64(&rlp, 3)?,
        to: decode_optional_address(&rlp, 4)?,
        value: decode_u256(&rlp, 5)?,
        data: decode_bytes(&rlp, 6)?,
        v: decode_u64(&rlp, 8)?, // yParity
        r: decode_bytes32(&rlp, 9)?,
        s: decode_bytes32(&rlp, 10)?,
    })
}

/// Parse EIP-1559 dynamic fee transaction
fn parse_dynamic_fee_tx(data: &[u8]) -> Result<ParsedTxFields, ApiError> {
    let rlp = Rlp::new(data);

    if !rlp.is_list() {
        return Err(ApiError::invalid_params("Transaction must be RLP list"));
    }

    let item_count = rlp.item_count().map_err(|e| rlp_error("item count", e))?;
    if item_count != 12 {
        return Err(ApiError::invalid_params(format!(
            "EIP-1559 transaction must have 12 fields, got {}",
            item_count
        )));
    }

    // EIP-1559: [chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList, yParity, r, s]
    let max_fee_per_gas = decode_u256(&rlp, 3)?;

    Ok(ParsedTxFields {
        nonce: decode_u64(&rlp, 1)?,
        gas_price: max_fee_per_gas, // Use maxFeePerGas as gas_price for prioritization
        gas_limit: decode_u64(&rlp, 4)?,
        to: decode_optional_address(&rlp, 5)?,
        value: decode_u256(&rlp, 6)?,
        data: decode_bytes(&rlp, 7)?,
        v: decode_u64(&rlp, 9)?, // yParity
        r: decode_bytes32(&rlp, 10)?,
        s: decode_bytes32(&rlp, 11)?,
    })
}

/// Compute transaction hash
fn compute_tx_hash(raw: &[u8], tx_type: TxType) -> Hash {
    let mut hasher = Keccak256::new();

    match tx_type {
        TxType::Legacy => {
            // Hash the entire RLP
            hasher.update(raw);
        }
        TxType::AccessList | TxType::DynamicFee => {
            // Hash includes the type byte prefix
            hasher.update(raw);
        }
    }

    let result = hasher.finalize();
    Hash::from_slice(&result)
}

/// Recover sender address from signature using secp256k1
fn recover_sender(
    tx: &ParsedTxFields,
    tx_type: TxType,
    rlp_data: &[u8],
) -> Result<Address, ApiError> {
    use secp256k1::{ecdsa::RecoverableSignature, Message, Secp256k1};

    // Validate signature components are non-zero
    if tx.r == [0u8; 32] || tx.s == [0u8; 32] {
        return Err(ApiError::invalid_params(
            "Invalid signature: r or s is zero",
        ));
    }

    // Validate s is in lower half of curve order (EIP-2)
    // secp256k1 curve order n = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
    let half_n: [u8; 32] = [
        0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B,
        0x20, 0xA0,
    ];
    if tx.s > half_n {
        return Err(ApiError::invalid_params(
            "Invalid signature: s value too high (EIP-2 violation)",
        ));
    }

    // Calculate recovery id and validate v value
    let recovery_id = match tx_type {
        TxType::Legacy => {
            // Legacy: v = 27/28 or EIP-155 encoded (v = chain_id * 2 + 35 + recovery_id)
            if tx.v == 27 || tx.v == 28 {
                (tx.v - 27) as i32
            } else if tx.v >= 35 {
                // EIP-155: recovery_id = (v - 35) % 2
                ((tx.v - 35) % 2) as i32
            } else {
                return Err(ApiError::invalid_params(format!(
                    "Invalid v value for legacy tx: {}",
                    tx.v
                )));
            }
        }
        TxType::AccessList | TxType::DynamicFee => {
            // EIP-2718: yParity should be 0 or 1
            if tx.v > 1 {
                return Err(ApiError::invalid_params(format!(
                    "Invalid yParity value: {}",
                    tx.v
                )));
            }
            tx.v as i32
        }
    };

    // Reconstruct the signing hash
    let signing_hash = compute_signing_hash(tx, tx_type, rlp_data)?;

    // Create recoverable signature
    let secp = Secp256k1::new();

    // Construct signature bytes (r || s)
    let mut sig_bytes = [0u8; 64];
    sig_bytes[..32].copy_from_slice(&tx.r);
    sig_bytes[32..].copy_from_slice(&tx.s);

    let rec_id = secp256k1::ecdsa::RecoveryId::from_i32(recovery_id)
        .map_err(|_| ApiError::invalid_params("Invalid recovery id"))?;

    let signature = RecoverableSignature::from_compact(&sig_bytes, rec_id)
        .map_err(|e| ApiError::invalid_params(format!("Invalid signature: {}", e)))?;

    let message = Message::from_digest_slice(&signing_hash)
        .map_err(|e| ApiError::invalid_params(format!("Invalid message hash: {}", e)))?;

    // Recover public key
    let public_key = secp
        .recover_ecdsa(&message, &signature)
        .map_err(|e| ApiError::invalid_params(format!("Signature recovery failed: {}", e)))?;

    // Derive address from public key (last 20 bytes of keccak256 of uncompressed pubkey without prefix)
    let pubkey_bytes = public_key.serialize_uncompressed();
    let mut hasher = Keccak256::new();
    hasher.update(&pubkey_bytes[1..]); // Skip the 0x04 prefix
    let hash = hasher.finalize();

    Ok(Address::from_slice(&hash[12..]))
}

/// Compute the signing hash for a transaction
fn compute_signing_hash(
    tx: &ParsedTxFields,
    tx_type: TxType,
    rlp_data: &[u8],
) -> Result<[u8; 32], ApiError> {
    let rlp = Rlp::new(rlp_data);

    match tx_type {
        TxType::Legacy => {
            // For legacy transactions, we need to reconstruct the unsigned tx
            // If EIP-155, include chain_id in the signing data
            let chain_id = if tx.v >= 35 {
                Some((tx.v - 35) / 2)
            } else {
                None
            };

            let mut stream = rlp::RlpStream::new_list(if chain_id.is_some() { 9 } else { 6 });
            stream.append(&tx.nonce);
            stream.append(&tx.gas_price.into_inner());
            stream.append(&tx.gas_limit);
            if let Some(to) = &tx.to {
                stream.append(to);
            } else {
                stream.append(&"");
            }
            stream.append(&tx.value.into_inner());
            stream.append(&tx.data);

            if let Some(chain_id) = chain_id {
                stream.append(&chain_id);
                stream.append(&0u8);
                stream.append(&0u8);
            }

            let mut hasher = Keccak256::new();
            hasher.update(stream.as_raw());
            Ok(hasher.finalize().into())
        }
        TxType::AccessList => {
            // EIP-2930: hash(0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList]))
            let chain_id: u64 = decode_u64(&rlp, 0)?;
            let access_list = decode_bytes(&rlp, 7)?;

            let mut stream = rlp::RlpStream::new_list(8);
            stream.append(&chain_id);
            stream.append(&tx.nonce);
            stream.append(&tx.gas_price.into_inner());
            stream.append(&tx.gas_limit);
            if let Some(to) = &tx.to {
                stream.append(to);
            } else {
                stream.append(&"");
            }
            stream.append(&tx.value.into_inner());
            stream.append(&tx.data);
            stream.append_raw(&access_list, 1);

            let mut hasher = Keccak256::new();
            hasher.update([0x01]); // Type prefix
            hasher.update(stream.as_raw());
            Ok(hasher.finalize().into())
        }
        TxType::DynamicFee => {
            // EIP-1559: hash(0x02 || rlp([chainId, nonce, maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList]))
            let chain_id: u64 = decode_u64(&rlp, 0)?;
            let max_priority_fee = decode_u256(&rlp, 2)?;
            let access_list = decode_bytes(&rlp, 8)?;

            let mut stream = rlp::RlpStream::new_list(9);
            stream.append(&chain_id);
            stream.append(&tx.nonce);
            stream.append(&max_priority_fee.into_inner());
            stream.append(&tx.gas_price.into_inner()); // maxFeePerGas
            stream.append(&tx.gas_limit);
            if let Some(to) = &tx.to {
                stream.append(to);
            } else {
                stream.append(&"");
            }
            stream.append(&tx.value.into_inner());
            stream.append(&tx.data);
            stream.append_raw(&access_list, 1);

            let mut hasher = Keccak256::new();
            hasher.update([0x02]); // Type prefix
            hasher.update(stream.as_raw());
            Ok(hasher.finalize().into())
        }
    }
}

// Helper functions for RLP decoding

fn decode_u64(rlp: &Rlp, index: usize) -> Result<u64, ApiError> {
    rlp.at(index)
        .and_then(|r| r.as_val())
        .map_err(|e| rlp_error(&format!("field {}", index), e))
}

fn decode_u256(rlp: &Rlp, index: usize) -> Result<U256, ApiError> {
    let bytes = decode_bytes(rlp, index)?;
    if bytes.len() > 32 {
        return Err(ApiError::invalid_params(format!(
            "U256 field {} too large: {} bytes",
            index,
            bytes.len()
        )));
    }

    let mut arr = [0u8; 32];
    arr[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(U256::from(primitive_types::U256::from_big_endian(&arr)))
}

fn decode_bytes(rlp: &Rlp, index: usize) -> Result<Vec<u8>, ApiError> {
    rlp.at(index)
        .and_then(|r| r.as_val::<Vec<u8>>())
        .map_err(|e| rlp_error(&format!("field {}", index), e))
}

fn decode_bytes32(rlp: &Rlp, index: usize) -> Result<[u8; 32], ApiError> {
    let bytes = decode_bytes(rlp, index)?;
    if bytes.len() > 32 {
        return Err(ApiError::invalid_params(format!(
            "bytes32 field {} too large: {} bytes",
            index,
            bytes.len()
        )));
    }

    let mut arr = [0u8; 32];
    arr[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(arr)
}

fn decode_optional_address(rlp: &Rlp, index: usize) -> Result<Option<Address>, ApiError> {
    let bytes = decode_bytes(rlp, index)?;
    if bytes.is_empty() {
        Ok(None)
    } else if bytes.len() == 20 {
        Ok(Some(Address::from_slice(&bytes)))
    } else {
        Err(ApiError::invalid_params(format!(
            "Invalid address length at field {}: {} bytes",
            index,
            bytes.len()
        )))
    }
}

fn rlp_error(field: &str, e: DecoderError) -> ApiError {
    ApiError::invalid_params(format!("RLP decode error for {}: {:?}", field, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_type_detection_legacy() {
        // Legacy tx starts with 0xf8 or higher (RLP list prefix)
        let legacy_tx = vec![0xf8, 0x65 /* rest of tx */];
        let (tx_type, data) = detect_tx_type(&legacy_tx).unwrap();
        assert_eq!(tx_type, TxType::Legacy);
        assert_eq!(data, &legacy_tx[..]);
    }

    #[test]
    fn test_tx_type_detection_eip1559() {
        // EIP-1559 tx starts with 0x02
        let eip1559_tx = vec![0x02, 0xf8, 0x65 /* rest of tx */];
        let (tx_type, data) = detect_tx_type(&eip1559_tx).unwrap();
        assert_eq!(tx_type, TxType::DynamicFee);
        assert_eq!(data, &eip1559_tx[1..]);
    }

    #[test]
    fn test_tx_type_detection_eip2930() {
        // EIP-2930 tx starts with 0x01
        let eip2930_tx = vec![0x01, 0xf8, 0x65 /* rest of tx */];
        let (tx_type, data) = detect_tx_type(&eip2930_tx).unwrap();
        assert_eq!(tx_type, TxType::AccessList);
        assert_eq!(data, &eip2930_tx[1..]);
    }

    #[test]
    fn test_unknown_tx_type() {
        // Unknown type 0x03
        let unknown_tx = vec![0x03, 0xf8, 0x65];
        let result = detect_tx_type(&unknown_tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_tx_rejected() {
        let result = validate_raw_transaction(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_tx_too_small() {
        let small_tx = vec![0xf8; 10];
        let result = validate_raw_transaction(&small_tx);
        assert!(result.is_err());
    }

    #[test]
    fn test_tx_too_large() {
        let large_tx = vec![0xf8; MAX_TX_SIZE + 1];
        let result = validate_raw_transaction(&large_tx);
        assert!(result.is_err());
    }
}
