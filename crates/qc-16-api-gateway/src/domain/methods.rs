//! Method tier classification and whitelist per SPEC-16 Section 3.
//!
//! Tier 1: Public (No Auth) - Read operations and pre-signed transactions
//! Tier 2: Protected (API Key / Local) - Node status and pool info
//! Tier 3: Admin (Localhost + Auth) - Node management

use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

/// Method access tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodTier {
    /// Public - no authentication required
    /// Read operations, pre-signed tx submission
    Public,
    /// Protected - requires API key OR localhost
    /// Node status, mempool info
    Protected,
    /// Admin - requires localhost AND API key
    /// Node management, debug methods
    Admin,
}

impl MethodTier {
    /// Check if tier requires authentication
    pub fn requires_auth(&self) -> bool {
        matches!(self, MethodTier::Protected | MethodTier::Admin)
    }

    /// Check if tier requires localhost
    pub fn requires_localhost(&self) -> bool {
        matches!(self, MethodTier::Admin)
    }
}

/// Method category for grouping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MethodCategory {
    Eth,
    Web3,
    Net,
    TxPool,
    Admin,
    Debug,
    Trace,
}

/// Method metadata
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// Full method name (e.g., "eth_getBalance")
    pub name: &'static str,
    /// Access tier
    pub tier: MethodTier,
    /// Category
    pub category: MethodCategory,
    /// Recommended timeout
    pub timeout: Duration,
    /// Is this a write operation?
    pub is_write: bool,
    /// Target subsystem
    pub target_subsystem: Option<&'static str>,
    /// Brief description
    pub description: &'static str,
}

impl MethodInfo {
    const fn new(
        name: &'static str,
        tier: MethodTier,
        category: MethodCategory,
        timeout_secs: u64,
        is_write: bool,
        target_subsystem: Option<&'static str>,
        description: &'static str,
    ) -> Self {
        Self {
            name,
            tier,
            category,
            timeout: Duration::from_secs(timeout_secs),
            is_write,
            target_subsystem,
            description,
        }
    }
}

/// Method registry - all supported methods with metadata
pub static METHOD_REGISTRY: LazyLock<HashMap<&'static str, MethodInfo>> = LazyLock::new(|| {
    let methods = [
        // ═══════════════════════════════════════════════════════════════════════
        // TIER 1: PUBLIC METHODS (No Auth Required)
        // ═══════════════════════════════════════════════════════════════════════

        // --- Chain Info ---
        MethodInfo::new(
            "eth_chainId",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Returns the chain ID",
        ),
        MethodInfo::new(
            "eth_blockNumber",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-02-block-storage"),
            "Returns current block number",
        ),
        MethodInfo::new(
            "eth_gasPrice",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-06-mempool"),
            "Returns current gas price",
        ),
        MethodInfo::new(
            "eth_maxPriorityFeePerGas",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-06-mempool"),
            "Returns max priority fee suggestion",
        ),
        MethodInfo::new(
            "eth_feeHistory",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-02-block-storage"),
            "Returns fee history",
        ),
        // --- Account State ---
        MethodInfo::new(
            "eth_getBalance",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-04-state-management"),
            "Returns account balance",
        ),
        MethodInfo::new(
            "eth_getCode",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-04-state-management"),
            "Returns contract code",
        ),
        MethodInfo::new(
            "eth_getStorageAt",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-04-state-management"),
            "Returns storage value at position",
        ),
        MethodInfo::new(
            "eth_getTransactionCount",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-04-state-management"),
            "Returns account nonce",
        ),
        MethodInfo::new(
            "eth_accounts",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Returns empty list (no managed accounts)",
        ),
        // --- Block Data ---
        MethodInfo::new(
            "eth_getBlockByHash",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-02-block-storage"),
            "Returns block by hash",
        ),
        MethodInfo::new(
            "eth_getBlockByNumber",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-02-block-storage"),
            "Returns block by number",
        ),
        MethodInfo::new(
            "eth_getBlockTransactionCountByHash",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-02-block-storage"),
            "Returns tx count in block by hash",
        ),
        MethodInfo::new(
            "eth_getBlockTransactionCountByNumber",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("qc-02-block-storage"),
            "Returns tx count in block by number",
        ),
        MethodInfo::new(
            "eth_getUncleCountByBlockHash",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Returns 0 (no uncles in PoS)",
        ),
        MethodInfo::new(
            "eth_getUncleCountByBlockNumber",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Returns 0 (no uncles in PoS)",
        ),
        // --- Transaction Data ---
        MethodInfo::new(
            "eth_getTransactionByHash",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns transaction by hash",
        ),
        MethodInfo::new(
            "eth_getTransactionByBlockHashAndIndex",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns tx by block hash and index",
        ),
        MethodInfo::new(
            "eth_getTransactionByBlockNumberAndIndex",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns tx by block number and index",
        ),
        MethodInfo::new(
            "eth_getTransactionReceipt",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns transaction receipt",
        ),
        MethodInfo::new(
            "eth_getBlockReceipts",
            MethodTier::Public,
            MethodCategory::Eth,
            30,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns all receipts for a block",
        ),
        // --- Execution ---
        MethodInfo::new(
            "eth_call",
            MethodTier::Public,
            MethodCategory::Eth,
            30,
            false,
            Some("qc-11-smart-contracts"),
            "Executes call without creating transaction",
        ),
        MethodInfo::new(
            "eth_estimateGas",
            MethodTier::Public,
            MethodCategory::Eth,
            30,
            false,
            Some("qc-11-smart-contracts"),
            "Estimates gas for transaction",
        ),
        MethodInfo::new(
            "eth_createAccessList",
            MethodTier::Public,
            MethodCategory::Eth,
            30,
            false,
            Some("qc-11-smart-contracts"),
            "Creates access list for transaction",
        ),
        // --- Transaction Submission ---
        MethodInfo::new(
            "eth_sendRawTransaction",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            true,
            Some("qc-06-mempool"),
            "Submits pre-signed transaction",
        ),
        // --- Logs & Events ---
        MethodInfo::new(
            "eth_getLogs",
            MethodTier::Public,
            MethodCategory::Eth,
            60,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns logs matching filter",
        ),
        MethodInfo::new(
            "eth_getFilterChanges",
            MethodTier::Public,
            MethodCategory::Eth,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns filter changes since last poll",
        ),
        MethodInfo::new(
            "eth_getFilterLogs",
            MethodTier::Public,
            MethodCategory::Eth,
            60,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns all logs for filter",
        ),
        MethodInfo::new(
            "eth_newFilter",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Creates log filter",
        ),
        MethodInfo::new(
            "eth_newBlockFilter",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Creates block filter",
        ),
        MethodInfo::new(
            "eth_newPendingTransactionFilter",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Creates pending tx filter",
        ),
        MethodInfo::new(
            "eth_uninstallFilter",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            None,
            "Removes filter",
        ),
        // --- Sync Status ---
        MethodInfo::new(
            "eth_syncing",
            MethodTier::Public,
            MethodCategory::Eth,
            5,
            false,
            Some("node-runtime"),
            "Returns sync status",
        ),
        // --- Web3 ---
        MethodInfo::new(
            "web3_clientVersion",
            MethodTier::Public,
            MethodCategory::Web3,
            5,
            false,
            None,
            "Returns client version",
        ),
        MethodInfo::new(
            "web3_sha3",
            MethodTier::Public,
            MethodCategory::Web3,
            5,
            false,
            None,
            "Returns Keccak-256 hash",
        ),
        // --- Net ---
        MethodInfo::new(
            "net_version",
            MethodTier::Public,
            MethodCategory::Net,
            5,
            false,
            None,
            "Returns network ID",
        ),
        MethodInfo::new(
            "net_listening",
            MethodTier::Public,
            MethodCategory::Net,
            5,
            false,
            Some("qc-01-peer-discovery"),
            "Returns true if listening",
        ),
        MethodInfo::new(
            "net_peerCount",
            MethodTier::Public,
            MethodCategory::Net,
            5,
            false,
            Some("qc-01-peer-discovery"),
            "Returns peer count",
        ),
        // ═══════════════════════════════════════════════════════════════════════
        // TIER 2: PROTECTED METHODS (API Key OR Localhost)
        // ═══════════════════════════════════════════════════════════════════════

        // --- TxPool ---
        MethodInfo::new(
            "txpool_status",
            MethodTier::Protected,
            MethodCategory::TxPool,
            5,
            false,
            Some("qc-06-mempool"),
            "Returns txpool status",
        ),
        MethodInfo::new(
            "txpool_content",
            MethodTier::Protected,
            MethodCategory::TxPool,
            30,
            false,
            Some("qc-06-mempool"),
            "Returns full txpool content",
        ),
        MethodInfo::new(
            "txpool_contentFrom",
            MethodTier::Protected,
            MethodCategory::TxPool,
            10,
            false,
            Some("qc-06-mempool"),
            "Returns txpool content for address",
        ),
        MethodInfo::new(
            "txpool_inspect",
            MethodTier::Protected,
            MethodCategory::TxPool,
            30,
            false,
            Some("qc-06-mempool"),
            "Returns txpool summary",
        ),
        // --- Admin Info (read-only) ---
        MethodInfo::new(
            "admin_nodeInfo",
            MethodTier::Protected,
            MethodCategory::Admin,
            5,
            false,
            Some("qc-01-peer-discovery"),
            "Returns node info",
        ),
        MethodInfo::new(
            "admin_peers",
            MethodTier::Protected,
            MethodCategory::Admin,
            5,
            false,
            Some("qc-01-peer-discovery"),
            "Returns connected peers",
        ),
        MethodInfo::new(
            "admin_datadir",
            MethodTier::Protected,
            MethodCategory::Admin,
            5,
            false,
            None,
            "Returns data directory path",
        ),
        // ═══════════════════════════════════════════════════════════════════════
        // TIER 3: ADMIN METHODS (Localhost AND Auth Required)
        // ═══════════════════════════════════════════════════════════════════════

        // --- Admin Control ---
        MethodInfo::new(
            "admin_addPeer",
            MethodTier::Admin,
            MethodCategory::Admin,
            10,
            true,
            Some("qc-01-peer-discovery"),
            "Adds a peer",
        ),
        MethodInfo::new(
            "admin_removePeer",
            MethodTier::Admin,
            MethodCategory::Admin,
            10,
            true,
            Some("qc-01-peer-discovery"),
            "Removes a peer",
        ),
        MethodInfo::new(
            "admin_addTrustedPeer",
            MethodTier::Admin,
            MethodCategory::Admin,
            10,
            true,
            Some("qc-01-peer-discovery"),
            "Adds trusted peer",
        ),
        MethodInfo::new(
            "admin_removeTrustedPeer",
            MethodTier::Admin,
            MethodCategory::Admin,
            10,
            true,
            Some("qc-01-peer-discovery"),
            "Removes trusted peer",
        ),
        // --- Debug ---
        MethodInfo::new(
            "debug_traceTransaction",
            MethodTier::Admin,
            MethodCategory::Debug,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Traces transaction execution",
        ),
        MethodInfo::new(
            "debug_traceBlockByHash",
            MethodTier::Admin,
            MethodCategory::Debug,
            300,
            false,
            Some("qc-11-smart-contracts"),
            "Traces all txs in block",
        ),
        MethodInfo::new(
            "debug_traceBlockByNumber",
            MethodTier::Admin,
            MethodCategory::Debug,
            300,
            false,
            Some("qc-11-smart-contracts"),
            "Traces all txs in block",
        ),
        MethodInfo::new(
            "debug_traceCall",
            MethodTier::Admin,
            MethodCategory::Debug,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Traces call without tx",
        ),
        MethodInfo::new(
            "debug_storageRangeAt",
            MethodTier::Admin,
            MethodCategory::Debug,
            30,
            false,
            Some("qc-04-state-management"),
            "Returns storage range",
        ),
        MethodInfo::new(
            "debug_accountRange",
            MethodTier::Admin,
            MethodCategory::Debug,
            30,
            false,
            Some("qc-04-state-management"),
            "Returns account range",
        ),
        MethodInfo::new(
            "debug_getHeaderRlp",
            MethodTier::Admin,
            MethodCategory::Debug,
            5,
            false,
            Some("qc-02-block-storage"),
            "Returns header RLP",
        ),
        MethodInfo::new(
            "debug_getBlockRlp",
            MethodTier::Admin,
            MethodCategory::Debug,
            10,
            false,
            Some("qc-02-block-storage"),
            "Returns block RLP",
        ),
        MethodInfo::new(
            "debug_setHead",
            MethodTier::Admin,
            MethodCategory::Debug,
            60,
            true,
            Some("qc-02-block-storage"),
            "Sets chain head (DANGEROUS)",
        ),
        MethodInfo::new(
            "debug_getRawBlock",
            MethodTier::Admin,
            MethodCategory::Debug,
            10,
            false,
            Some("qc-02-block-storage"),
            "Returns raw block bytes",
        ),
        MethodInfo::new(
            "debug_getRawHeader",
            MethodTier::Admin,
            MethodCategory::Debug,
            5,
            false,
            Some("qc-02-block-storage"),
            "Returns raw header bytes",
        ),
        MethodInfo::new(
            "debug_getRawReceipts",
            MethodTier::Admin,
            MethodCategory::Debug,
            10,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns raw receipts",
        ),
        MethodInfo::new(
            "debug_getRawTransaction",
            MethodTier::Admin,
            MethodCategory::Debug,
            5,
            false,
            Some("qc-03-transaction-indexing"),
            "Returns raw tx bytes",
        ),
        // --- Trace (for advanced debugging) ---
        MethodInfo::new(
            "trace_block",
            MethodTier::Admin,
            MethodCategory::Trace,
            300,
            false,
            Some("qc-11-smart-contracts"),
            "Returns traces for block",
        ),
        MethodInfo::new(
            "trace_transaction",
            MethodTier::Admin,
            MethodCategory::Trace,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Returns traces for tx",
        ),
        MethodInfo::new(
            "trace_call",
            MethodTier::Admin,
            MethodCategory::Trace,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Traces call",
        ),
        MethodInfo::new(
            "trace_callMany",
            MethodTier::Admin,
            MethodCategory::Trace,
            300,
            false,
            Some("qc-11-smart-contracts"),
            "Traces multiple calls",
        ),
        MethodInfo::new(
            "trace_rawTransaction",
            MethodTier::Admin,
            MethodCategory::Trace,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Traces raw tx",
        ),
        MethodInfo::new(
            "trace_replayBlockTransactions",
            MethodTier::Admin,
            MethodCategory::Trace,
            300,
            false,
            Some("qc-11-smart-contracts"),
            "Replays block txs with trace",
        ),
        MethodInfo::new(
            "trace_replayTransaction",
            MethodTier::Admin,
            MethodCategory::Trace,
            120,
            false,
            Some("qc-11-smart-contracts"),
            "Replays tx with trace",
        ),
    ];

    methods.into_iter().map(|m| (m.name, m)).collect()
});

/// Get method info by name
pub fn get_method_info(method: &str) -> Option<&'static MethodInfo> {
    METHOD_REGISTRY.get(method)
}

/// Check if method is supported
pub fn is_method_supported(method: &str) -> bool {
    METHOD_REGISTRY.contains_key(method)
}

/// Get method tier
pub fn get_method_tier(method: &str) -> Option<MethodTier> {
    METHOD_REGISTRY.get(method).map(|m| m.tier)
}

/// Get method timeout
pub fn get_method_timeout(method: &str) -> Duration {
    METHOD_REGISTRY
        .get(method)
        .map(|m| m.timeout)
        .unwrap_or(Duration::from_secs(10))
}

/// Check if method is a write operation
pub fn is_write_method(method: &str) -> bool {
    METHOD_REGISTRY
        .get(method)
        .map(|m| m.is_write)
        .unwrap_or(false)
}

/// Get all methods for a tier
pub fn get_methods_by_tier(tier: MethodTier) -> Vec<&'static str> {
    METHOD_REGISTRY
        .values()
        .filter(|m| m.tier == tier)
        .map(|m| m.name)
        .collect()
}

/// Get all methods for a category
pub fn get_methods_by_category(category: MethodCategory) -> Vec<&'static str> {
    METHOD_REGISTRY
        .values()
        .filter(|m| m.category == category)
        .map(|m| m.name)
        .collect()
}

/// Subscription types for WebSocket
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubscriptionType {
    NewHeads,
    Logs,
    NewPendingTransactions,
    Syncing,
}

impl SubscriptionType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "newHeads" => Some(SubscriptionType::NewHeads),
            "logs" => Some(SubscriptionType::Logs),
            "newPendingTransactions" => Some(SubscriptionType::NewPendingTransactions),
            "syncing" => Some(SubscriptionType::Syncing),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionType::NewHeads => "newHeads",
            SubscriptionType::Logs => "logs",
            SubscriptionType::NewPendingTransactions => "newPendingTransactions",
            SubscriptionType::Syncing => "syncing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_registry() {
        assert!(is_method_supported("eth_getBalance"));
        assert!(is_method_supported("eth_sendRawTransaction"));
        assert!(!is_method_supported("eth_fakeMethod"));
    }

    #[test]
    fn test_method_tiers() {
        assert_eq!(get_method_tier("eth_getBalance"), Some(MethodTier::Public));
        assert_eq!(
            get_method_tier("txpool_status"),
            Some(MethodTier::Protected)
        );
        assert_eq!(get_method_tier("admin_addPeer"), Some(MethodTier::Admin));
    }

    #[test]
    fn test_write_methods() {
        assert!(!is_write_method("eth_getBalance"));
        assert!(is_write_method("eth_sendRawTransaction"));
        assert!(is_write_method("admin_addPeer"));
    }

    #[test]
    fn test_get_methods_by_tier() {
        let public = get_methods_by_tier(MethodTier::Public);
        assert!(public.contains(&"eth_getBalance"));
        assert!(public.contains(&"eth_sendRawTransaction"));
        assert!(!public.contains(&"admin_addPeer"));

        let admin = get_methods_by_tier(MethodTier::Admin);
        assert!(admin.contains(&"admin_addPeer"));
        assert!(admin.contains(&"debug_traceTransaction"));
    }

    #[test]
    fn test_method_timeouts() {
        assert_eq!(get_method_timeout("eth_chainId"), Duration::from_secs(5));
        assert_eq!(get_method_timeout("eth_call"), Duration::from_secs(30));
        assert_eq!(get_method_timeout("eth_getLogs"), Duration::from_secs(60));
    }

    #[test]
    fn test_subscription_types() {
        assert_eq!(
            SubscriptionType::from_str("newHeads"),
            Some(SubscriptionType::NewHeads)
        );
        assert_eq!(SubscriptionType::from_str("invalid"), None);
    }
}
