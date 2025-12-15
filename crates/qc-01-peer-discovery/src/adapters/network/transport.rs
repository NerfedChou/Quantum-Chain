use crate::domain::SocketAddr;
use crate::ports::{NetworkError, NetworkSocket};

// ============================================================================
// NoOpNetworkSocket - Stub for testing without network
// ============================================================================

/// No-operation network socket for testing.
///
/// All operations succeed but don't send any actual packets.
/// Use this for unit testing domain logic without network dependencies.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpNetworkSocket;

impl NoOpNetworkSocket {
    /// Create a new no-op socket.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl NetworkSocket for NoOpNetworkSocket {
    fn send_ping(&self, _target: SocketAddr) -> Result<(), NetworkError> {
        Ok(())
    }

    fn send_find_node(
        &self,
        _target: SocketAddr,
        _search_id: crate::domain::NodeId,
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    fn send_pong(&self, _target: SocketAddr) -> Result<(), NetworkError> {
        Ok(())
    }
}

// ============================================================================
// UdpNetworkSocket - Production UDP Socket (requires "network" feature)
// ============================================================================

/// Kademlia message types for UDP protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    /// PING request to check if peer is alive.
    Ping = 0x01,
    /// PONG response to a PING.
    Pong = 0x02,
    /// Request to find nodes close to a target ID.
    FindNode = 0x03,
    /// Response containing list of nodes.
    Nodes = 0x04,
    /// Bootstrap request with identity proof.
    Bootstrap = 0x05,
}

#[cfg(feature = "network")]
mod udp_socket {
    use super::*;
    use crate::domain::{IpAddr, NodeId};
    use std::net::UdpSocket as StdUdpSocket;
    use std::sync::Arc;

    /// UDP-based network socket for Kademlia protocol.
    ///
    /// This adapter implements the `NetworkSocket` port using standard UDP.
    /// It wraps a `std::net::UdpSocket` for synchronous sends.
    ///
    /// # Wire Protocol
    ///
    /// Messages are simple binary format:
    /// - Byte 0: Message type (PING=0x01, PONG=0x02, FIND_NODE=0x03, NODES=0x04)
    /// - Bytes 1-32: Our NodeId (for PING/PONG/FIND_NODE)
    /// - Bytes 33-64: Target NodeId (for FIND_NODE only)
    /// - For BOOTSTRAP (0x05):
    ///   - Bytes 1-32: NodeId
    ///   - Bytes 33-64: Proof of Work
    ///   - Bytes 65-97: Claimed PubKey
    ///   - Bytes 98-161: Signature (64 bytes)
    ///   - Bytes 162-163: Port
    ///   - Remaining: IP Address (4 or 16 bytes)
    pub struct UdpNetworkSocket {
        socket: Arc<StdUdpSocket>,
        local_node_id: NodeId,
    }

    impl UdpNetworkSocket {
        /// Bind to a local address and create the socket.
        ///
        /// # Arguments
        ///
        /// * `bind_addr` - Local address to bind (e.g., "0.0.0.0:8080")
        /// * `local_node_id` - Our NodeId to include in messages
        ///
        /// # Errors
        ///
        /// Returns error if socket binding fails.
        pub fn bind(bind_addr: &str, local_node_id: NodeId) -> std::io::Result<Self> {
            let socket = StdUdpSocket::bind(bind_addr)?;
            socket.set_nonblocking(true)?;
            Ok(Self {
                socket: Arc::new(socket),
                local_node_id,
            })
        }

        /// Get the local address the socket is bound to.
        pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
            self.socket.local_addr()
        }

        /// Convert domain IpAddr to std::net::IpAddr.
        fn to_std_ip(ip: IpAddr) -> std::net::IpAddr {
            match ip {
                IpAddr::V4(bytes) => std::net::IpAddr::V4(std::net::Ipv4Addr::from(bytes)),
                IpAddr::V6(bytes) => std::net::IpAddr::V6(std::net::Ipv6Addr::from(bytes)),
            }
        }

        /// Convert domain SocketAddr to std::net::SocketAddr.
        fn to_std_addr(addr: SocketAddr) -> std::net::SocketAddr {
            std::net::SocketAddr::new(Self::to_std_ip(addr.ip), addr.port)
        }

        /// Send raw bytes to a target address.
        fn send_to(&self, data: &[u8], target: SocketAddr) -> Result<(), NetworkError> {
            let std_addr = Self::to_std_addr(target);
            match self.socket.send_to(data, std_addr) {
                Ok(_n) => Ok(()),
                Err(e) => match e.kind() {
                    std::io::ErrorKind::WouldBlock => Err(NetworkError::Timeout),
                    std::io::ErrorKind::ConnectionRefused => Err(NetworkError::ConnectionRefused),
                    std::io::ErrorKind::InvalidInput => Err(NetworkError::InvalidAddress),
                    _ => Err(NetworkError::Timeout),
                },
            }
        }
    }

    impl NetworkSocket for UdpNetworkSocket {
        fn send_ping(&self, target: SocketAddr) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)]
            let mut msg = [0u8; 33];
            msg[0] = MessageType::Ping as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            self.send_to(&msg, target)
        }

        fn send_find_node(
            &self,
            target: SocketAddr,
            search_id: NodeId,
        ) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)] [search_id(32)]
            let mut msg = [0u8; 65];
            msg[0] = MessageType::FindNode as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            msg[33..65].copy_from_slice(search_id.as_bytes());
            self.send_to(&msg, target)
        }

        fn send_pong(&self, target: SocketAddr) -> Result<(), NetworkError> {
            // Wire format: [type(1)] [our_node_id(32)]
            let mut msg = [0u8; 33];
            msg[0] = MessageType::Pong as u8;
            msg[1..33].copy_from_slice(self.local_node_id.as_bytes());
            self.send_to(&msg, target)
        }
    }

    // Allow cloning the socket handle
    impl Clone for UdpNetworkSocket {
        fn clone(&self) -> Self {
            Self {
                socket: Arc::clone(&self.socket),
                local_node_id: self.local_node_id,
            }
        }
    }
}

#[cfg(feature = "network")]
pub use udp_socket::UdpNetworkSocket;
