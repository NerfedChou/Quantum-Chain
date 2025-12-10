//! # Proposer-Builder Separation (PBS)
//!
//! Decouples block building from block proposing to prevent MEV extraction.
//!
//! ## Problem
//!
//! Validators who build blocks can reorder transactions for profit (MEV),
//! leading to centralization and unfair advantages.
//!
//! ## Solution: Commit-Reveal Payload
//!
//! 1. Builders create blinded payload headers + bid value
//! 2. Proposer commits to highest bid (without seeing transactions)
//! 3. Builder reveals full payload after seeing commitment
//! 4. If builder fails to reveal, they are slashed
//!
//! Reference: SPEC-08-CONSENSUS.md Phase 4

use crate::domain::ValidatorId;
use shared_types::{Hash, U256};
use std::collections::HashMap;

/// Builder identifier.
pub type BuilderId = [u8; 32];

/// Blinded execution payload header.
///
/// Contains commitment to transaction ordering without revealing contents.
#[derive(Clone, Debug)]
pub struct PayloadHeader {
    /// Builder who created this payload
    pub builder_id: BuilderId,
    /// Hash of the full payload (blind commitment)
    pub payload_hash: Hash,
    /// Bid value in wei
    pub bid_value: U256,
    /// Gas limit for this payload
    pub gas_limit: u64,
    /// Gas used estimate
    pub gas_used: u64,
    /// Block number this is for
    pub block_number: u64,
    /// Parent block hash
    pub parent_hash: Hash,
    /// Timestamp
    pub timestamp: u64,
}

/// Full execution payload (revealed after commitment).
#[derive(Clone, Debug)]
pub struct ExecutionPayload {
    /// Header this payload corresponds to
    pub header: PayloadHeader,
    /// Full transaction list (only revealed after commit)
    pub transactions: Vec<Vec<u8>>,
    /// Withdrawals
    pub withdrawals: Vec<Withdrawal>,
}

/// Withdrawal from consensus layer.
#[derive(Clone, Debug)]
pub struct Withdrawal {
    pub index: u64,
    pub validator_index: u64,
    pub address: [u8; 20],
    pub amount: u64,
}

/// Commitment from proposer to a specific payload.
#[derive(Clone, Debug)]
pub struct PayloadCommitment {
    /// Slot/block this is for
    pub slot: u64,
    /// Proposer who made the commitment
    pub proposer: ValidatorId,
    /// Header being committed to
    pub header: PayloadHeader,
    /// Proposer's signature on the header
    pub signature: Vec<u8>,
    /// Timestamp of commitment
    pub committed_at: u64,
}

/// PBS auction state for a single slot.
#[derive(Debug)]
pub struct SlotAuction {
    /// Slot number
    pub slot: u64,
    /// All bids received
    pub bids: Vec<PayloadHeader>,
    /// Winning bid (if selected)
    pub winning_bid: Option<PayloadHeader>,
    /// Proposer commitment (if made)
    pub commitment: Option<PayloadCommitment>,
    /// Revealed payload (if received)
    pub revealed_payload: Option<ExecutionPayload>,
    /// Auction deadline
    pub deadline: u64,
}

impl SlotAuction {
    pub fn new(slot: u64, deadline: u64) -> Self {
        Self {
            slot,
            bids: Vec::new(),
            winning_bid: None,
            commitment: None,
            revealed_payload: None,
            deadline,
        }
    }

    /// Submit a bid from a builder.
    pub fn submit_bid(&mut self, header: PayloadHeader) -> Result<(), PbsError> {
        if header.block_number != self.slot {
            return Err(PbsError::WrongSlot);
        }
        self.bids.push(header);
        Ok(())
    }

    /// Get the highest bid.
    pub fn highest_bid(&self) -> Option<&PayloadHeader> {
        self.bids.iter().max_by_key(|h| h.bid_value)
    }

    /// Proposer commits to a bid.
    pub fn commit(
        &mut self,
        proposer: ValidatorId,
        header: PayloadHeader,
        signature: Vec<u8>,
        timestamp: u64,
    ) -> Result<(), PbsError> {
        if self.commitment.is_some() {
            return Err(PbsError::AlreadyCommitted);
        }

        self.winning_bid = Some(header.clone());
        self.commitment = Some(PayloadCommitment {
            slot: self.slot,
            proposer,
            header,
            signature,
            committed_at: timestamp,
        });
        Ok(())
    }

    /// Builder reveals the full payload.
    pub fn reveal(&mut self, payload: ExecutionPayload) -> Result<(), PbsError> {
        let commitment = self.commitment.as_ref().ok_or(PbsError::NoCommitment)?;

        // Verify payload matches commitment
        if payload.header.payload_hash != commitment.header.payload_hash {
            return Err(PbsError::PayloadMismatch);
        }

        // Verify actual payload hash
        let computed_hash = compute_payload_hash(&payload);
        if computed_hash != payload.header.payload_hash {
            return Err(PbsError::InvalidPayloadHash);
        }

        self.revealed_payload = Some(payload);
        Ok(())
    }

    /// Check if auction is complete (has revealed payload).
    pub fn is_complete(&self) -> bool {
        self.revealed_payload.is_some()
    }

    /// Check if builder failed to reveal (slashable).
    pub fn is_builder_slashable(&self, current_time: u64) -> bool {
        self.commitment.is_some()
            && self.revealed_payload.is_none()
            && current_time > self.deadline
    }
}

/// Compute hash of execution payload.
fn compute_payload_hash(payload: &ExecutionPayload) -> Hash {
    use sha3::{Digest, Keccak256};
    let mut hasher = Keccak256::new();
    
    for tx in &payload.transactions {
        hasher.update(tx);
    }
    hasher.update(payload.header.gas_limit.to_le_bytes());
    hasher.update(payload.header.gas_used.to_le_bytes());
    
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// PBS service managing auctions across slots.
#[derive(Debug, Default)]
pub struct PbsService {
    /// Active auctions by slot
    auctions: HashMap<u64, SlotAuction>,
    /// Auction duration in milliseconds
    auction_duration_ms: u64,
}

impl PbsService {
    pub fn new(auction_duration_ms: u64) -> Self {
        Self {
            auctions: HashMap::new(),
            auction_duration_ms,
        }
    }

    /// Start auction for a slot.
    pub fn start_auction(&mut self, slot: u64, start_time: u64) {
        let deadline = start_time + self.auction_duration_ms;
        self.auctions.insert(slot, SlotAuction::new(slot, deadline));
    }

    /// Get auction for slot.
    pub fn get_auction(&self, slot: u64) -> Option<&SlotAuction> {
        self.auctions.get(&slot)
    }

    /// Get mutable auction for slot.
    pub fn get_auction_mut(&mut self, slot: u64) -> Option<&mut SlotAuction> {
        self.auctions.get_mut(&slot)
    }

    /// Clean up old auctions.
    pub fn cleanup_before(&mut self, slot: u64) {
        self.auctions.retain(|&s, _| s >= slot);
    }
}

/// PBS errors.
#[derive(Clone, Debug, PartialEq)]
pub enum PbsError {
    WrongSlot,
    AlreadyCommitted,
    NoCommitment,
    PayloadMismatch,
    InvalidPayloadHash,
    AuctionNotFound,
    BuilderSlashed,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_header(slot: u64, bid: u64) -> PayloadHeader {
        PayloadHeader {
            builder_id: [1; 32],
            payload_hash: [0xAB; 32],
            bid_value: U256::from(bid),
            gas_limit: 30_000_000,
            gas_used: 15_000_000,
            block_number: slot,
            parent_hash: [0; 32],
            timestamp: 1000,
        }
    }

    #[test]
    fn test_submit_bid() {
        let mut auction = SlotAuction::new(100, 2000);
        
        let header = make_header(100, 1000);
        assert!(auction.submit_bid(header).is_ok());
        assert_eq!(auction.bids.len(), 1);
    }

    #[test]
    fn test_highest_bid() {
        let mut auction = SlotAuction::new(100, 2000);
        
        auction.submit_bid(make_header(100, 500)).unwrap();
        auction.submit_bid(make_header(100, 1500)).unwrap();
        auction.submit_bid(make_header(100, 1000)).unwrap();
        
        let highest = auction.highest_bid().unwrap();
        assert_eq!(highest.bid_value, U256::from(1500u64));
    }

    #[test]
    fn test_commit() {
        let mut auction = SlotAuction::new(100, 2000);
        let header = make_header(100, 1000);
        
        auction.commit([0; 32], header.clone(), vec![1, 2, 3], 1500).unwrap();
        
        assert!(auction.commitment.is_some());
        assert!(auction.winning_bid.is_some());
    }

    #[test]
    fn test_double_commit_fails() {
        let mut auction = SlotAuction::new(100, 2000);
        let header = make_header(100, 1000);
        
        auction.commit([0; 32], header.clone(), vec![], 1500).unwrap();
        let result = auction.commit([0; 32], header, vec![], 1600);
        
        assert_eq!(result, Err(PbsError::AlreadyCommitted));
    }

    #[test]
    fn test_builder_slashable() {
        let mut auction = SlotAuction::new(100, 2000);
        let header = make_header(100, 1000);
        
        auction.commit([0; 32], header, vec![], 1500).unwrap();
        
        // Before deadline - not slashable
        assert!(!auction.is_builder_slashable(1999));
        
        // After deadline - slashable
        assert!(auction.is_builder_slashable(2001));
    }

    #[test]
    fn test_pbs_service() {
        let mut service = PbsService::new(1000);
        
        service.start_auction(100, 5000);
        service.start_auction(101, 6000);
        
        assert!(service.get_auction(100).is_some());
        assert!(service.get_auction(101).is_some());
        
        service.cleanup_before(101);
        assert!(service.get_auction(100).is_none());
        assert!(service.get_auction(101).is_some());
    }
}
