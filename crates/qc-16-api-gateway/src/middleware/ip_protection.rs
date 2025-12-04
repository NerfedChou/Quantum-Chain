//! IP spoofing protection middleware per SPEC-16 Section 7.5.
//!
//! Prevents IP spoofing via X-Forwarded-For header manipulation.
//! Only trusted proxies can set forwarded headers.

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{HeaderValue, Request},
    response::Response,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tower::{Layer, Service};
use tracing::{debug, warn};

/// Trusted proxy configuration
#[derive(Clone, Debug)]
pub struct TrustedProxyConfig {
    /// List of trusted proxy IPs
    pub trusted_proxies: Vec<IpAddr>,
    /// Trust local IPs (127.0.0.1, ::1)
    pub trust_localhost: bool,
    /// Trust private IPs (10.x.x.x, 192.168.x.x, 172.16-31.x.x)
    pub trust_private: bool,
    /// Header to use for real IP (X-Forwarded-For, X-Real-IP, etc.)
    pub real_ip_header: String,
    /// Number of trusted proxies in chain (for X-Forwarded-For)
    pub proxy_count: usize,
}

impl Default for TrustedProxyConfig {
    fn default() -> Self {
        Self {
            trusted_proxies: Vec::new(),
            trust_localhost: true,
            trust_private: false, // Default to not trusting private - security first
            real_ip_header: "X-Forwarded-For".to_string(),
            proxy_count: 1,
        }
    }
}

/// IP protection layer
#[derive(Clone)]
pub struct IpProtectionLayer {
    config: Arc<TrustedProxyConfig>,
}

impl IpProtectionLayer {
    pub fn new(config: TrustedProxyConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Create with default settings (trust localhost only)
    pub fn localhost_only() -> Self {
        Self::new(TrustedProxyConfig::default())
    }

    /// Create with no trusted proxies (always use direct connection IP)
    pub fn direct_only() -> Self {
        Self::new(TrustedProxyConfig {
            trusted_proxies: Vec::new(),
            trust_localhost: false,
            trust_private: false,
            real_ip_header: String::new(),
            proxy_count: 0,
        })
    }
}

impl<S> Layer<S> for IpProtectionLayer {
    type Service = IpProtectionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IpProtectionService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// IP protection service
#[derive(Clone)]
pub struct IpProtectionService<S> {
    inner: S,
    config: Arc<TrustedProxyConfig>,
}

impl<S> Service<Request<Body>> for IpProtectionService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let config = Arc::clone(&self.config);
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Get direct connection IP
            let direct_ip = req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0.ip())
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));

            // Determine real client IP
            let real_ip = determine_real_ip(&req, direct_ip, &config);

            // Store real IP in extension for downstream middleware
            let (mut parts, body) = req.into_parts();

            // Add real IP as header for downstream use
            if let Ok(ip_str) = HeaderValue::from_str(&real_ip.to_string()) {
                parts.headers.insert("x-real-client-ip", ip_str);
            }

            // Log if we detected potential spoofing attempt
            if !is_trusted_proxy(direct_ip, &config) {
                if let Some(forwarded) = parts.headers.get("x-forwarded-for") {
                    warn!(
                        direct_ip = %direct_ip,
                        forwarded = ?forwarded,
                        "Ignoring X-Forwarded-For from untrusted source"
                    );
                }
            }

            let req = Request::from_parts(parts, body);
            inner.call(req).await
        })
    }
}

/// Determine the real client IP based on trusted proxy configuration
fn determine_real_ip(
    req: &Request<Body>,
    direct_ip: IpAddr,
    config: &TrustedProxyConfig,
) -> IpAddr {
    // If direct connection is not from a trusted proxy, use direct IP
    if !is_trusted_proxy(direct_ip, config) {
        return direct_ip;
    }

    // Try to get real IP from configured header
    if !config.real_ip_header.is_empty() {
        if let Some(header_value) = req.headers().get(&config.real_ip_header) {
            if let Ok(value_str) = header_value.to_str() {
                if config
                    .real_ip_header
                    .eq_ignore_ascii_case("x-forwarded-for")
                {
                    // X-Forwarded-For can have multiple IPs: client, proxy1, proxy2
                    // Take the Nth from the right based on proxy_count
                    let ips: Vec<&str> = value_str.split(',').map(|s| s.trim()).collect();
                    let index = ips.len().saturating_sub(config.proxy_count + 1);
                    if let Some(ip_str) = ips.get(index) {
                        if let Ok(ip) = ip_str.parse::<IpAddr>() {
                            debug!(
                                header = config.real_ip_header,
                                value = value_str,
                                extracted_ip = %ip,
                                "Extracted client IP from header"
                            );
                            return ip;
                        }
                    }
                } else {
                    // Other headers (X-Real-IP) are single value
                    if let Ok(ip) = value_str.trim().parse::<IpAddr>() {
                        return ip;
                    }
                }
            }
        }
    }

    // Fall back to direct IP
    direct_ip
}

/// Check if an IP is a trusted proxy
fn is_trusted_proxy(ip: IpAddr, config: &TrustedProxyConfig) -> bool {
    // Check explicit trusted list
    if config.trusted_proxies.contains(&ip) {
        return true;
    }

    // Check localhost
    if config.trust_localhost && is_localhost(ip) {
        return true;
    }

    // Check private networks
    if config.trust_private && is_private_ip(ip) {
        return true;
    }

    false
}

/// Check if IP is localhost
fn is_localhost(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_loopback(),
        IpAddr::V6(ipv6) => ipv6.is_loopback(),
    }
}

/// Check if IP is in private range
fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private() || ipv4.is_link_local(),
        IpAddr::V6(ipv6) => {
            // IPv6 unique local addresses (fc00::/7)
            let octets = ipv6.octets();
            (octets[0] & 0xfe) == 0xfc
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    #[test]
    fn test_is_localhost() {
        assert!(is_localhost(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_localhost(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(!is_localhost(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_localhost(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }

    #[test]
    fn test_is_private_ip() {
        // Private ranges
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));

        // Public IPs
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn test_trusted_proxy_localhost() {
        let config = TrustedProxyConfig {
            trust_localhost: true,
            ..Default::default()
        };
        assert!(is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            &config
        ));
        assert!(!is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)),
            &config
        ));
    }

    #[test]
    fn test_trusted_proxy_explicit() {
        let config = TrustedProxyConfig {
            trusted_proxies: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100))],
            trust_localhost: false,
            trust_private: false,
            ..Default::default()
        };
        assert!(is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 100)),
            &config
        ));
        assert!(!is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 101)),
            &config
        ));
        assert!(!is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            &config
        ));
    }

    #[test]
    fn test_no_trusted_proxies() {
        let config = TrustedProxyConfig {
            trusted_proxies: Vec::new(),
            trust_localhost: false,
            trust_private: false,
            ..Default::default()
        };
        assert!(!is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            &config
        ));
        assert!(!is_trusted_proxy(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            &config
        ));
    }
}
