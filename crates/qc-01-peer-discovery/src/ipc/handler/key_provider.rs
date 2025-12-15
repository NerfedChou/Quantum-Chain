//! Static Key Provider for IPC security.

use shared_types::security::KeyProvider;
use std::collections::HashMap;

/// Static key provider using pre-configured shared secrets.
///
/// Maps each subsystem ID (1-15) to its HMAC shared secret for message
/// authentication per Architecture.md Section 3.5. Production deployments
/// load secrets from environment variables via `NodeConfig`.
///
/// Reference: Architecture.md Section 7.1 (Defense in Depth - Layer 3: IPC Security)
#[derive(Clone)]
pub struct StaticKeyProvider {
    /// HMAC-SHA256 shared secrets indexed by subsystem ID (1-15).
    secrets: HashMap<u8, Vec<u8>>,
}

impl StaticKeyProvider {
    /// Create a new key provider with a default shared secret for all subsystems.
    #[must_use]
    pub fn new(default_secret: &[u8]) -> Self {
        let mut secrets = HashMap::new();
        // Pre-populate with secrets for authorized senders per IPC-MATRIX
        for id in 1..=15 {
            secrets.insert(id, default_secret.to_vec());
        }
        Self { secrets }
    }

    /// Create a key provider with specific per-subsystem secrets.
    #[must_use]
    pub fn with_secrets(secrets: HashMap<u8, Vec<u8>>) -> Self {
        Self { secrets }
    }
}

impl KeyProvider for StaticKeyProvider {
    fn get_shared_secret(&self, sender_id: u8) -> Option<Vec<u8>> {
        self.secrets.get(&sender_id).cloned()
    }
}
