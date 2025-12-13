//! Authentication middleware per SPEC-16 Section 7.4.
//!
//! Enforces method tier restrictions based on API key and localhost status.

use crate::ApiError;
use crate::{get_method_tier, MethodTier};
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
    response::Response,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tower::{Layer, Service};
use tracing::{debug, warn};

/// Authentication configuration
#[derive(Clone, Default)]
pub struct AuthConfig {
    /// API key for protected/admin access (None = no key required)
    pub api_key: Option<String>,
    /// Allow admin access from non-localhost (DANGEROUS)
    pub allow_external_admin: bool,
}

/// Authentication layer
#[derive(Clone)]
pub struct AuthLayer {
    config: Arc<AuthConfig>,
}

impl AuthLayer {
    pub fn new(config: AuthConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

/// Authentication service
#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
    config: Arc<AuthConfig>,
}

impl<S> Service<Request<Body>> for AuthService<S>
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
            // Extract method from request if possible
            // Note: This is a simplified check - full implementation would parse JSON-RPC
            let method = extract_method_from_request(&req);

            if let Some(method_name) = &method {
                let tier = get_method_tier(method_name).unwrap_or(MethodTier::Admin);
                let is_localhost = is_request_from_localhost(&req);
                let has_valid_key = check_api_key(&req, &config);

                debug!(
                    method = method_name,
                    tier = ?tier,
                    is_localhost = is_localhost,
                    has_valid_key = has_valid_key,
                    "Checking method authorization"
                );

                // Check authorization based on tier
                match tier {
                    MethodTier::Public => {
                        // Always allowed
                    }
                    MethodTier::Protected => {
                        // Requires API key OR localhost
                        if !has_valid_key && !is_localhost {
                            warn!(
                                method = method_name,
                                "Protected method access denied - requires API key or localhost"
                            );
                            return Ok(unauthorized_response(
                                "Protected method requires API key or localhost access",
                            ));
                        }
                    }
                    MethodTier::Admin => {
                        // Requires localhost (unless allow_external_admin) AND API key (if configured)
                        if !is_localhost && !config.allow_external_admin {
                            warn!(
                                method = method_name,
                                "Admin method access denied - localhost required"
                            );
                            return Ok(unauthorized_response(
                                "Admin method requires localhost access",
                            ));
                        }

                        if config.api_key.is_some() && !has_valid_key {
                            warn!(
                                method = method_name,
                                "Admin method access denied - API key required"
                            );
                            return Ok(unauthorized_response("Admin method requires API key"));
                        }
                    }
                }
            }

            // Authorized - proceed with request
            inner.call(req).await
        })
    }
}

/// Check if request is from localhost
fn is_request_from_localhost<B>(req: &Request<B>) -> bool {
    // Try to get from ConnectInfo
    if let Some(connect_info) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        return is_localhost_ip(connect_info.0.ip());
    }

    // Check X-Forwarded-For (be careful with this in production)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return is_localhost_ip(ip);
                }
            }
        }
    }

    // Default to false for safety
    false
}

/// Check if IP is localhost
fn is_localhost_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4 == Ipv4Addr::LOCALHOST || ipv4 == Ipv4Addr::new(127, 0, 0, 1),
        IpAddr::V6(ipv6) => {
            ipv6 == Ipv6Addr::LOCALHOST || ipv6 == Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)
        }
    }
}

/// Check API key from request
fn check_api_key<B>(req: &Request<B>, config: &AuthConfig) -> bool {
    let expected_key = match &config.api_key {
        Some(key) => key,
        None => return true, // No key configured = always valid
    };

    // Check Authorization header (Bearer token)
    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return constant_time_compare(token, expected_key);
            }
        }
    }

    // Check X-API-Key header
    if let Some(api_key) = req.headers().get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            return constant_time_compare(key_str, expected_key);
        }
    }

    // Check query parameter (less secure, for debugging)
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some(key) = pair.strip_prefix("api_key=") {
                return constant_time_compare(key, expected_key);
            }
        }
    }

    false
}

/// Constant-time string comparison to prevent timing attacks
///
/// SECURITY: This function takes the same amount of time regardless of how
/// many characters match, preventing timing side-channel attacks.
///
/// IMPORTANT: We use `subtle::ConstantTimeEq` for proper constant-time comparison.
/// The naive XOR approach can still be optimized by the compiler.
pub fn constant_time_compare(a: &str, b: &str) -> bool {
    use subtle::ConstantTimeEq;

    // Pad shorter string to match length (prevents length oracle)
    // We compare both padded to the MAX of both lengths
    let max_len = std::cmp::max(a.len(), b.len());

    // Create padded versions - pad with different bytes to ensure inequality
    let mut a_padded = vec![0u8; max_len];
    let mut b_padded = vec![0xFFu8; max_len]; // Different pad value ensures mismatch if lengths differ

    a_padded[..a.len()].copy_from_slice(a.as_bytes());
    b_padded[..b.len()].copy_from_slice(b.as_bytes());

    // Use subtle crate for true constant-time comparison
    // AND check that lengths match (also in constant time)
    let lengths_equal = a.len().ct_eq(&b.len());
    let contents_equal = a_padded.ct_eq(&b_padded);

    // Both conditions must be true
    (lengths_equal & contents_equal).into()
}

/// Extract method name from request (simplified)
fn extract_method_from_request<B>(req: &Request<B>) -> Option<String> {
    // Check custom header set by earlier middleware
    if let Some(method) = req.headers().get("x-rpc-method") {
        if let Ok(method_str) = method.to_str() {
            return Some(method_str.to_string());
        }
    }

    // Could also parse from query string for GET requests
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some(method) = pair.strip_prefix("method=") {
                return Some(method.to_string());
            }
        }
    }

    None
}

/// Create unauthorized response
fn unauthorized_response(message: &str) -> Response {
    let error = ApiError::unauthorized(message);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "error": error,
        "id": null
    });

    let mut response = Response::new(Body::from(serde_json::to_vec(&body).unwrap_or_default()));
    *response.status_mut() = StatusCode::UNAUTHORIZED;
    response
        .headers_mut()
        .insert("Content-Type", "application/json".parse().unwrap());
    response
        .headers_mut()
        .insert("WWW-Authenticate", "Bearer".parse().unwrap());

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_ip_detection() {
        assert!(is_localhost_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_localhost_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_localhost_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        assert!(!is_localhost_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(!is_localhost_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn test_constant_time_compare() {
        assert!(constant_time_compare("secret", "secret"));
        assert!(!constant_time_compare("secret", "Secret"));
        assert!(!constant_time_compare("secret", "secre"));
        assert!(!constant_time_compare("secret", "secrets"));
    }

    #[test]
    fn test_api_key_check_bearer() {
        let config = AuthConfig {
            api_key: Some("test-key-123".to_string()),
            allow_external_admin: false,
        };

        let req = Request::builder()
            .header("Authorization", "Bearer test-key-123")
            .body(Body::empty())
            .unwrap();

        assert!(check_api_key(&req, &config));

        // Wrong key
        let req_wrong = Request::builder()
            .header("Authorization", "Bearer wrong-key")
            .body(Body::empty())
            .unwrap();
        assert!(!check_api_key(&req_wrong, &config));
    }

    #[test]
    fn test_api_key_check_header() {
        let config = AuthConfig {
            api_key: Some("test-key-123".to_string()),
            allow_external_admin: false,
        };

        let req = Request::builder()
            .header("X-API-Key", "test-key-123")
            .body(Body::empty())
            .unwrap();

        assert!(check_api_key(&req, &config));
    }

    #[test]
    fn test_no_api_key_configured() {
        let config = AuthConfig {
            api_key: None,
            allow_external_admin: false,
        };

        let req = Request::builder().body(Body::empty()).unwrap();

        // No key configured = always valid
        assert!(check_api_key(&req, &config));
    }
}
