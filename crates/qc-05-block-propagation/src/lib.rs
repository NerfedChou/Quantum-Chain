//! # Block Propagation Subsystem (QC-05)
//!
//! **Version:** 1.0.0  
//! **Specification:** SPEC-05-BLOCK-PROPAGATION.md v2.3  
//! **Architecture:** Hexagonal (Ports & Adapters) per Architecture.md v2.3
//!
//! ## Overview
//!
//! Distributes validated blocks across the P2P network using an epidemic gossip protocol.
//! Implements BIP152-style compact block relay for bandwidth efficiency.
//!
//! ## Architecture Role (System.md)
//!
//! Block Propagation is a **Level 3** subsystem in the dependency hierarchy:
//!
//! ```text
//! [Consensus (8)] ──PropagateBlockRequest──→ [Block Propagation (5)]
//!                                                    │
//!                                                    ↓ gossip (fanout=8)
//!                                            ┌───────┴───────┐
//!                                            ↓               ↓
//!                                       [Peer A]        [Peer B] ...
//!                                            │               │
//!                                   CompactBlock      CompactBlock
//! ```
//!
//! ## Domain Invariants
//!
//! | ID | Invariant | Enforcement Location |
//! |----|-----------|---------------------|
//! | INVARIANT-1 | Deduplication | `domain/invariants.rs` - `invariant_no_duplicate_processing()` |
//! | INVARIANT-2 | Rate Limiting | `domain/invariants.rs` - `invariant_rate_limit()` |
//! | INVARIANT-3 | Size Limit | `domain/invariants.rs` - `invariant_size_limit()` |
//!
//! All invariants are combined in `check_all_invariants()` for efficient validation.
//!
//! ## Security (IPC-MATRIX.md)
//!
//! - **Centralized Security**: Uses `MessageVerifier` from `shared-types` crate
//! - **Envelope-Only Identity**: Identity derived solely from `AuthenticatedMessage.sender_id`
//! - **Replay Prevention**: Nonce tracking via centralized `NonceCache`
//!
//! ### IPC Authorization Matrix
//!
//! | Operation | Authorized Sender(s) | Enforcement |
//! |-----------|---------------------|-------------|
//! | `PropagateBlockRequest` | Consensus (8) only | `ipc/handler.rs:77-80` |
//!
//! ### Network Security
//!
//! | Rule | Description | Enforcement |
//! |------|-------------|-------------|
//! | **Signature Verification** | All network blocks verified via Subsystem 10 | `service.rs:453-467` |
//! | **Invalid Signatures** | SILENT DROP (IP spoofing defense) | `service.rs:461-467` |
//! | **Rate Limiting** | Max 1 announcement per peer per second | `service.rs:283-286` |
//! | **Size Limit** | Blocks >10MB are rejected | `service.rs:181-186` |
//!
//! ## Outbound Dependencies
//!
//! | Subsystem | Trait | Purpose |
//! |-----------|-------|---------|
//! | 1 (Peer Discovery) | `PeerNetwork` | Get connected peers for gossip |
//! | 6 (Mempool) | `MempoolGateway` | Transaction lookup for compact blocks |
//! | 8 (Consensus) | `ConsensusGateway` | Submit validated blocks |
//! | 10 (Sig Verify) | `SignatureVerifier` | Block signature verification |
//!
//! ## Module Structure (Hexagonal Architecture)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      OUTER LAYER                                │
//! │  adapters/ - Port implementations (in node-runtime)            │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ implements ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      MIDDLE LAYER                               │
//! │  ports/inbound.rs  - BlockPropagationApi, BlockReceiver        │
//! │  ports/outbound.rs - PeerNetwork, ConsensusGateway, etc.       │
//! └─────────────────────────────────────────────────────────────────┘
//!                          ↑ uses ↑
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      INNER LAYER                                │
//! │  domain/entities.rs      - BlockAnnouncement, CompactBlock     │
//! │  domain/value_objects.rs - PropagationConfig, SeenBlockCache   │
//! │  domain/services.rs      - calculate_short_id, reconstruct     │
//! │  domain/invariants.rs    - Security invariant checks           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Compact Block Relay (BIP152)
//!
//! This implementation supports compact block relay for bandwidth optimization:
//!
//! 1. **Short Transaction IDs**: 6-byte SipHash of transaction hashes
//! 2. **Prefilled Transactions**: Coinbase always included
//! 3. **Mempool Lookup**: Missing transactions fetched from local mempool
//! 4. **Fallback**: Full block requested if reconstruction fails
//!
//! **Note:** Current version operates in fallback mode - compact blocks are
//! transmitted but reconstruction always requests the full block. This is
//! secure and correct; bandwidth optimization will be enhanced in v1.1.
//!
//! ## Usage Example
//!
//! ```rust,ignore
//! use qc_05_block_propagation::{BlockPropagationService, PropagationConfig};
//!
//! // Create service with adapters (provided by node-runtime)
//! let service = BlockPropagationService::new(
//!     PropagationConfig::default(),
//!     network_adapter,
//!     consensus_adapter,
//!     mempool_adapter,
//!     signature_adapter,
//! );
//!
//! // Propagate a validated block
//! let stats = service.propagate_block(block_hash, block_data, tx_hashes)?;
//! println!("Block sent to {} peers", stats.peers_reached);
//! ```
//!
//! ## IPC Message Handling
//!
//! ```rust,ignore
//! use qc_05_block_propagation::ipc::IpcHandler;
//!
//! let handler = IpcHandler::new(service, master_secret);
//!
//! // Handle authenticated message from Consensus
//! handler.handle_propagate_block(authenticated_message, &raw_bytes)?;
//! ```


pub mod domain;
pub mod events;
pub mod ipc;
pub mod ports;
pub mod service;

// Re-export primary types for convenience
pub use domain::{
    BlockAnnouncement, CompactBlock, PeerId, PeerPropagationState, PrefilledTx, PropagationConfig,
    PropagationMetrics, PropagationState, PropagationStats, SeenBlockCache, ShortTxId,
};
pub use events::PropagationError;
pub use ports::inbound::{BlockPropagationApi, BlockReceiver};
pub use service::BlockPropagationService;
