//! Core types for the API Gateway with proper serialization.
//!
//! All types follow Ethereum JSON-RPC conventions with hex string serialization.

use primitive_types::U256 as PrimitiveU256;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

// Re-export primitive types for convenience
pub use primitive_types::{H160 as Address, H256 as Hash};

/// Block number type (u64)
pub type BlockNumber = u64;

/// U256 wrapper with hex string serialization for JSON-RPC compatibility.
///
/// Serializes as `"0x..."` hex string, deserializes from hex string or number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct U256(pub PrimitiveU256);

impl U256 {
    pub const ZERO: U256 = U256(PrimitiveU256::zero());
    pub const ONE: U256 = U256(PrimitiveU256::one());
    pub const MAX: U256 = U256(PrimitiveU256::MAX);

    #[inline]
    pub fn from_dec_str(s: &str) -> Result<Self, &'static str> {
        PrimitiveU256::from_dec_str(s)
            .map(U256)
            .map_err(|_| "invalid decimal string")
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[inline]
    pub fn as_u64(&self) -> u64 {
        self.0.as_u64()
    }

    #[inline]
    pub fn as_u128(&self) -> u128 {
        self.0.as_u128()
    }

    #[inline]
    pub fn inner(&self) -> &PrimitiveU256 {
        &self.0
    }

    #[inline]
    pub fn into_inner(self) -> PrimitiveU256 {
        self.0
    }

    /// Saturating addition
    #[inline]
    pub fn saturating_add(self, other: Self) -> Self {
        U256(self.0.saturating_add(other.0))
    }

    /// Saturating subtraction
    #[inline]
    pub fn saturating_sub(self, other: Self) -> Self {
        U256(self.0.saturating_sub(other.0))
    }

    /// Saturating multiplication
    #[inline]
    pub fn saturating_mul(self, other: Self) -> Self {
        U256(self.0.saturating_mul(other.0))
    }

    /// Checked division
    #[inline]
    pub fn checked_div(self, other: Self) -> Option<Self> {
        self.0.checked_div(other.0).map(U256)
    }
}

impl From<u64> for U256 {
    fn from(v: u64) -> Self {
        U256(PrimitiveU256::from(v))
    }
}

impl From<u128> for U256 {
    fn from(v: u128) -> Self {
        U256(PrimitiveU256::from(v))
    }
}

impl From<PrimitiveU256> for U256 {
    fn from(v: PrimitiveU256) -> Self {
        U256(v)
    }
}

impl From<U256> for PrimitiveU256 {
    fn from(v: U256) -> Self {
        v.0
    }
}

impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

impl fmt::LowerHex for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl Serialize for U256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Always serialize as hex string with 0x prefix
        serializer.serialize_str(&format!("0x{:x}", self.0))
    }
}

impl<'de> Deserialize<'de> for U256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U256Visitor;

        impl<'de> de::Visitor<'de> for U256Visitor {
            type Value = U256;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a hex string starting with 0x or a number")
            }

            fn visit_str<E>(self, value: &str) -> Result<U256, E>
            where
                E: de::Error,
            {
                if let Some(hex_str) = value
                    .strip_prefix("0x")
                    .or_else(|| value.strip_prefix("0X"))
                {
                    PrimitiveU256::from_str(hex_str)
                        .map(U256)
                        .map_err(|_| de::Error::custom("invalid hex string for U256"))
                } else {
                    // Try decimal
                    PrimitiveU256::from_dec_str(value)
                        .map(U256)
                        .map_err(|_| de::Error::custom("invalid decimal string for U256"))
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<U256, E>
            where
                E: de::Error,
            {
                Ok(U256::from(value))
            }

            fn visit_u128<E>(self, value: u128) -> Result<U256, E>
            where
                E: de::Error,
            {
                Ok(U256::from(value))
            }
        }

        deserializer.deserialize_any(U256Visitor)
    }
}

/// Block identifier for JSON-RPC requests.
///
/// Can be a block number, hash, or tag like "latest", "pending", "earliest".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BlockId {
    /// Block number as hex
    Number(BlockNumber),
    /// Block hash
    Hash(BlockHashOrNumber),
    /// Block tag
    Tag(BlockTag),
}

impl Default for BlockId {
    fn default() -> Self {
        BlockId::Tag(BlockTag::Latest)
    }
}

/// Block hash with optional requirement for canonical chain
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHashOrNumber {
    #[serde(rename = "blockHash", skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<Hash>,
    #[serde(rename = "blockNumber", skip_serializing_if = "Option::is_none")]
    pub block_number: Option<BlockNumber>,
    #[serde(rename = "requireCanonical", default)]
    pub require_canonical: bool,
}

/// Block tags for JSON-RPC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockTag {
    #[default]
    Latest,
    Earliest,
    Pending,
    Safe,
    Finalized,
}

impl Serialize for BlockTag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for BlockTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BlockTag::from_str(&s).map_err(de::Error::custom)
    }
}

impl BlockTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockTag::Latest => "latest",
            BlockTag::Earliest => "earliest",
            BlockTag::Pending => "pending",
            BlockTag::Safe => "safe",
            BlockTag::Finalized => "finalized",
        }
    }
}

impl FromStr for BlockTag {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "latest" => Ok(BlockTag::Latest),
            "earliest" => Ok(BlockTag::Earliest),
            "pending" => Ok(BlockTag::Pending),
            "safe" => Ok(BlockTag::Safe),
            "finalized" => Ok(BlockTag::Finalized),
            _ => Err("invalid block tag"),
        }
    }
}

/// Transaction call object for eth_call and eth_estimateGas
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallRequest {
    /// Sender address (optional for eth_call)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Address>,
    /// Target address (None for contract creation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<Address>,
    /// Gas limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<U256>,
    /// Gas price (legacy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<U256>,
    /// Max fee per gas (EIP-1559)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_fee_per_gas: Option<U256>,
    /// Max priority fee per gas (EIP-1559)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_priority_fee_per_gas: Option<U256>,
    /// Value to transfer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    /// Input data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    /// Input data (alias for data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Bytes>,
    /// Nonce
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<U256>,
    /// Access list (EIP-2930)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
    /// Transaction type
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub transaction_type: Option<u64>,
}

/// Access list item for EIP-2930
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessListItem {
    pub address: Address,
    pub storage_keys: Vec<Hash>,
}

/// Bytes wrapper with hex serialization
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    pub fn new() -> Self {
        Bytes(Vec::new())
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        Bytes(slice.to_vec())
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(v: Vec<u8>) -> Self {
        Bytes(v)
    }
}

impl From<&[u8]> for Bytes {
    fn from(v: &[u8]) -> Self {
        Bytes(v.to_vec())
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{}", hex::encode(&self.0)))
    }
}

impl<'de> Deserialize<'de> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        hex::decode(s)
            .map(Bytes)
            .map_err(|_| de::Error::custom("invalid hex bytes"))
    }
}

/// Filter for eth_getLogs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filter {
    /// From block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_block: Option<BlockId>,
    /// To block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_block: Option<BlockId>,
    /// Contract addresses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<FilterAddress>,
    /// Topics (up to 4)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<Option<FilterTopic>>>,
    /// Block hash (alternative to from_block/to_block)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<Hash>,
}

/// Filter address - single or multiple
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterAddress {
    Single(Address),
    Multiple(Vec<Address>),
}

/// Filter topic - single or multiple
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FilterTopic {
    Single(Hash),
    Multiple(Vec<Hash>),
}

/// Syncing status response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SyncStatus {
    /// Not syncing
    NotSyncing(bool),
    /// Syncing with progress
    Syncing(SyncProgress),
}

/// Sync progress details
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub starting_block: U256,
    pub current_block: U256,
    pub highest_block: U256,
}

/// Fee history response for eth_feeHistory (EIP-1559)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeHistory {
    /// Oldest block number in the returned range
    pub oldest_block: U256,
    /// Array of base fees per gas for each block
    pub base_fee_per_gas: Vec<U256>,
    /// Array of gas used ratios (0-1) for each block
    pub gas_used_ratio: Vec<f64>,
    /// Array of arrays of priority fees at requested percentiles
    /// Only present if reward_percentiles was provided in request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reward: Option<Vec<Vec<U256>>>,
    /// Array of blob base fees per gas for each block (EIP-4844)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_base_fee_per_gas: Option<Vec<U256>>,
    /// Array of blob gas used ratios for each block (EIP-4844)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_gas_used_ratio: Option<Vec<f64>>,
}

/// JSON-RPC request ID type
///
/// Per JSON-RPC 2.0 spec, ID can be string, number, or null.
/// We reject null IDs as they indicate notifications (no response expected).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    /// String ID
    String(String),
    /// Numeric ID (must fit in i64 for compatibility)
    Number(i64),
}

impl JsonRpcId {
    /// Validate the ID is acceptable
    ///
    /// Rejects:
    /// - Empty strings
    /// - Strings longer than 256 chars (DoS protection)
    pub fn validate(&self) -> Result<(), &'static str> {
        match self {
            JsonRpcId::String(s) => {
                if s.is_empty() {
                    Err("request ID cannot be empty string")
                } else if s.len() > 256 {
                    Err("request ID string too long (max 256 chars)")
                } else {
                    Ok(())
                }
            }
            JsonRpcId::Number(_) => Ok(()),
        }
    }
}

impl fmt::Display for JsonRpcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonRpcId::String(s) => write!(f, "\"{}\"", s),
            JsonRpcId::Number(n) => write!(f, "{}", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u256_serialize() {
        let val = U256::from(255u64);
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, "\"0xff\"");
    }

    #[test]
    fn test_u256_deserialize_hex() {
        let val: U256 = serde_json::from_str("\"0xff\"").unwrap();
        assert_eq!(val, U256::from(255u64));
    }

    #[test]
    fn test_u256_deserialize_decimal() {
        let val: U256 = serde_json::from_str("\"255\"").unwrap();
        assert_eq!(val, U256::from(255u64));
    }

    #[test]
    fn test_u256_deserialize_number() {
        let val: U256 = serde_json::from_str("255").unwrap();
        assert_eq!(val, U256::from(255u64));
    }

    #[test]
    fn test_bytes_serialize() {
        let bytes = Bytes::from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        let json = serde_json::to_string(&bytes).unwrap();
        assert_eq!(json, "\"0xdeadbeef\"");
    }

    #[test]
    fn test_bytes_deserialize() {
        let bytes: Bytes = serde_json::from_str("\"0xdeadbeef\"").unwrap();
        assert_eq!(bytes.as_slice(), &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_block_tag_serialize() {
        assert_eq!(
            serde_json::to_string(&BlockTag::Latest).unwrap(),
            "\"latest\""
        );
        assert_eq!(
            serde_json::to_string(&BlockTag::Finalized).unwrap(),
            "\"finalized\""
        );
    }

    #[test]
    fn test_block_tag_deserialize() {
        let tag: BlockTag = serde_json::from_str("\"latest\"").unwrap();
        assert_eq!(tag, BlockTag::Latest);
    }
}
