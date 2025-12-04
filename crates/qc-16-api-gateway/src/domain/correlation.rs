//! Correlation ID for request tracking.
//!
//! Uses UUID v7 for time-ordered, unique identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Correlation ID for tracking requests through the system.
///
/// Uses UUID v7 which is time-ordered, making it ideal for:
/// - Distributed tracing
/// - Log correlation
/// - Request/response matching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(Uuid);

impl CorrelationId {
    /// Generate a new correlation ID (UUID v7)
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Create from existing UUID
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Parse from string
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(s).map(Self)
    }

    /// Get the underlying UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Get as bytes
    pub fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }

    /// Get timestamp from UUID v7 (milliseconds since Unix epoch)
    pub fn timestamp_ms(&self) -> Option<u64> {
        // UUID v7 encodes timestamp in first 48 bits
        let bytes = self.0.as_bytes();
        if (bytes[6] >> 4) == 7 {
            // Version 7
            let ts = ((bytes[0] as u64) << 40)
                | ((bytes[1] as u64) << 32)
                | ((bytes[2] as u64) << 24)
                | ((bytes[3] as u64) << 16)
                | ((bytes[4] as u64) << 8)
                | (bytes[5] as u64);
            Some(ts)
        } else {
            None
        }
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Uuid> for CorrelationId {
    fn from(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl From<CorrelationId> for Uuid {
    fn from(id: CorrelationId) -> Self {
        id.0
    }
}

impl AsRef<Uuid> for CorrelationId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_correlation_id() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_correlation_id_serialization() {
        let id = CorrelationId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: CorrelationId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_correlation_id_display() {
        let id = CorrelationId::new();
        let display = id.to_string();
        assert_eq!(display.len(), 36); // UUID format: 8-4-4-4-12
    }

    #[test]
    fn test_parse_correlation_id() {
        let id = CorrelationId::new();
        let s = id.to_string();
        let parsed = CorrelationId::parse(&s).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_timestamp_extraction() {
        let id = CorrelationId::new();
        let ts = id.timestamp_ms();
        assert!(ts.is_some());
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        // Should be within 1 second
        assert!((ts.unwrap() as i64 - now_ms as i64).abs() < 1000);
    }
}
