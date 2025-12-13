//! Peer Discovery Adapter
//!
//! Implements `PeerDiscovery` port for finding full nodes.
//! Reference: SPEC-13 Section 3.2

use crate::domain::LightClientError;
use crate::ports::outbound::{FullNodeConnection, PeerDiscovery};
use crate::adapters::HttpFullNodeConnection;
use async_trait::async_trait;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info};

/// Peer discovery adapter using Peer Discovery subsystem (qc-01).
///
/// Per System.md Line 648: "Random peer selection from diverse sources"
pub struct PeerDiscoveryAdapter {
    /// Known node URLs (bootstrap nodes).
    bootstrap_nodes: Vec<String>,
    /// Currently active connections.
    active_connections: Arc<RwLock<Vec<Arc<dyn FullNodeConnection>>>>,
    /// Maximum connections to maintain.
    max_connections: usize,
}

impl PeerDiscoveryAdapter {
    /// Create a new adapter with bootstrap nodes.
    pub fn new(bootstrap_nodes: Vec<String>) -> Self {
        Self {
            bootstrap_nodes,
            active_connections: Arc::new(RwLock::new(Vec::new())),
            max_connections: 10,
        }
    }

    /// Create with custom max connections.
    pub fn with_max_connections(bootstrap_nodes: Vec<String>, max: usize) -> Self {
        Self {
            bootstrap_nodes,
            active_connections: Arc::new(RwLock::new(Vec::new())),
            max_connections: max,
        }
    }

    /// Add a static node.
    pub fn add_static_node(&mut self, url: String) {
        self.bootstrap_nodes.push(url);
    }
}

impl Default for PeerDiscoveryAdapter {
    fn default() -> Self {
        Self::new(vec![
            "http://localhost:8545".to_string(),
            "http://localhost:8546".to_string(),
            "http://localhost:8547".to_string(),
        ])
    }
}

#[async_trait]
impl PeerDiscovery for PeerDiscoveryAdapter {
    async fn get_full_nodes(
        &self,
        count: usize,
    ) -> Result<Vec<Box<dyn FullNodeConnection>>, LightClientError> {
        info!("[qc-13] Discovering {} full nodes", count);

        let count = count.min(self.bootstrap_nodes.len());
        
        // TODO: Query qc-01 Peer Discovery for diverse nodes
        // For now, use bootstrap nodes

        let nodes: Vec<Box<dyn FullNodeConnection>> = self
            .bootstrap_nodes
            .iter()
            .take(count)
            .enumerate()
            .map(|(i, url)| {
                let node_id = format!("node-{}", i);
                debug!("[qc-13] Creating connection to {} ({})", url, node_id);
                Box::new(HttpFullNodeConnection::new(url.clone(), node_id))
                    as Box<dyn FullNodeConnection>
            })
            .collect();

        if nodes.is_empty() {
            return Err(LightClientError::NoNodesAvailable);
        }

        Ok(nodes)
    }

    async fn rotate_peers(&mut self) -> Result<(), LightClientError> {
        info!("[qc-13] Rotating peers for privacy");

        // Clear current connections
        self.active_connections.write().clear();

        // TODO: Request new peers from qc-01 with diversity requirements
        // For now, shuffle bootstrap nodes (would need rand crate)

        debug!(
            "[qc-13] Peer rotation complete, {} bootstrap nodes available",
            self.bootstrap_nodes.len()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_full_nodes() {
        let adapter = PeerDiscoveryAdapter::default();
        let nodes = adapter.get_full_nodes(2).await.unwrap();

        assert_eq!(nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_rotate_peers() {
        let mut adapter = PeerDiscoveryAdapter::default();
        let result = adapter.rotate_peers().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_empty_bootstrap_fails() {
        let adapter = PeerDiscoveryAdapter::new(vec![]);
        let result = adapter.get_full_nodes(1).await;

        assert!(result.is_err());
    }
}
