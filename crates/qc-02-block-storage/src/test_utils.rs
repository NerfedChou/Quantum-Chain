use crate::ipc::envelope::{
    compute_message_signature, subsystem_ids, AuthenticatedMessage, SignatureContext, Topic,
};
use shared_types::{BlockHeader, ConsensusProof, ValidatedBlock, U256};

pub fn make_test_block(height: u64, parent_hash: [u8; 32]) -> ValidatedBlock {
    ValidatedBlock {
        header: BlockHeader {
            version: 1,
            height,
            parent_hash,
            merkle_root: [0; 32],
            state_root: [0; 32],
            timestamp: 1000 + height,
            proposer: [0xAA; 32],
            difficulty: U256::from(2).pow(U256::from(252)),
            nonce: 0,
        },
        transactions: vec![],
        consensus_proof: ConsensusProof {
            block_hash: [height as u8; 32],
            attestations: vec![],
            total_stake: 0,
        },
    }
}

pub fn compute_test_block_hash(block: &ValidatedBlock) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(block.header.version.to_le_bytes());
    hasher.update(block.header.height.to_le_bytes());
    hasher.update(block.header.parent_hash);
    hasher.update(block.header.merkle_root);
    hasher.update(block.header.state_root);
    hasher.update(block.header.timestamp.to_le_bytes());
    hasher.update(block.header.proposer);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub const ZERO_HASH: [u8; 32] = [0; 32];

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub struct MessageBuilder<T> {
    version: u8,
    sender: u8,
    recipient: u8,
    timestamp: u64,
    nonce: u64,
    reply_to: Option<Topic>,
    shared_secret: [u8; 32],
    payload: T,
}

impl MessageBuilder<()> {
    pub fn new() -> Self {
        Self {
            version: 1,
            sender: subsystem_ids::CONSENSUS,
            recipient: subsystem_ids::BLOCK_STORAGE,
            timestamp: current_timestamp(),
            nonce: 1,
            reply_to: None,
            shared_secret: [0u8; 32],
            payload: (),
        }
    }
}

impl<T> MessageBuilder<T>
where
    T: Clone,
{
    pub fn with_payload<U>(self, payload: U) -> MessageBuilder<U> {
        MessageBuilder {
            version: self.version,
            sender: self.sender,
            recipient: self.recipient,
            timestamp: self.timestamp,
            nonce: self.nonce,
            reply_to: self.reply_to,
            shared_secret: self.shared_secret,
            payload,
        }
    }

    pub fn version(mut self, v: u8) -> Self {
        self.version = v;
        self
    }

    pub fn sender(mut self, id: u8) -> Self {
        self.sender = id;
        self
    }

    pub fn recipient(mut self, id: u8) -> Self {
        self.recipient = id;
        self
    }

    pub fn timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn nonce(mut self, n: u64) -> Self {
        self.nonce = n;
        self
    }

    pub fn reply_to(mut self, r: Topic) -> Self {
        self.reply_to = Some(r);
        self
    }

    pub fn build(self) -> AuthenticatedMessage<T> {
        let mut msg = AuthenticatedMessage {
            version: self.version,
            correlation_id: [0; 16],
            reply_to: self.reply_to,
            sender_id: self.sender,
            recipient_id: self.recipient,
            timestamp: self.timestamp,
            nonce: self.nonce,
            signature: [0; 32],
            payload: self.payload.clone(),
        };

        // Compute valid signature
        let ctx = SignatureContext {
            shared_secret: &self.shared_secret,
            version: msg.version,
            correlation_id: &msg.correlation_id,
            sender_id: msg.sender_id,
            recipient_id: msg.recipient_id,
            timestamp: msg.timestamp,
            nonce: msg.nonce,
        };
        msg.signature = compute_message_signature(ctx);
        msg
    }
}
