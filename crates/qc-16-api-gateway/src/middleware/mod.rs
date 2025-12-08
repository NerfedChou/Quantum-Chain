//! Middleware stack for the API Gateway per SPEC-16 Section 7.
//!
//! Layer order: Request → IpProtection → RateLimit → Whitelist → Validation → Auth → Timeout → Tracing → Handler
//!
//! ## Circuit Breaker
//!
//! The circuit breaker pattern prevents cascading failures when downstream
//! subsystems become unhealthy. It tracks failures per-subsystem and opens
//! the circuit when a threshold is exceeded, rejecting requests immediately
//! until the service recovers.

pub mod auth;
pub mod circuit_breaker;
pub mod cors;
pub mod ip_protection;
pub mod metrics;
pub mod rate_limit;
pub mod timeout;
pub mod tracing;
pub mod validation;
pub mod whitelist;

pub use auth::{constant_time_compare, AuthConfig, AuthLayer};
pub use circuit_breaker::{CircuitBreakerConfig, CircuitBreakerManager, CircuitState, CircuitStats};
pub use cors::create_cors_layer;
pub use ip_protection::{IpProtectionLayer, TrustedProxyConfig};
pub use metrics::{GatewayMetrics, RequestTimer};
pub use rate_limit::{RateLimitLayer, RateLimitState};
pub use timeout::TimeoutLayer;
pub use tracing::TracingLayer;
pub use validation::{validate_jsonrpc, ValidationLayer};
pub use whitelist::{WhitelistConfig, WhitelistLayer};

use crate::domain::config::GatewayConfig;
use std::sync::Arc;

/// Middleware stack builder
pub struct MiddlewareStack {
    pub ip_protection: IpProtectionLayer,
    pub rate_limit: RateLimitLayer,
    pub whitelist: WhitelistLayer,
    pub validation: ValidationLayer,
    pub auth: AuthLayer,
    pub timeout: TimeoutLayer,
    pub tracing: TracingLayer,
    pub metrics: Arc<GatewayMetrics>,
    pub circuit_breaker: Arc<CircuitBreakerManager>,
}

impl MiddlewareStack {
    /// Create middleware stack from gateway config
    pub fn from_config(config: &GatewayConfig) -> Self {
        Self {
            ip_protection: IpProtectionLayer::new(TrustedProxyConfig {
                trusted_proxies: config.security.trusted_proxies.clone(),
                trust_localhost: true,
                trust_private: config.security.trust_private_ips,
                real_ip_header: "X-Forwarded-For".to_string(),
                proxy_count: config.security.proxy_count,
            }),
            rate_limit: RateLimitLayer::new(config.rate_limit.clone()),
            whitelist: WhitelistLayer::new(WhitelistConfig {
                allow_unknown: config.methods.allow_unknown,
                extra_methods: config.methods.extra_methods.clone(),
            }),
            validation: ValidationLayer::new(config.limits.clone()),
            auth: AuthLayer::new(AuthConfig {
                api_key: config.admin.api_key.clone(),
                allow_external_admin: config.admin.allow_external,
            }),
            timeout: TimeoutLayer::new(config.timeouts.clone()),
            tracing: TracingLayer::new(),
            metrics: Arc::new(GatewayMetrics::new()),
            circuit_breaker: Arc::new(CircuitBreakerManager::new(config.circuit_breaker.to_middleware_config())),
        }
    }

    /// Get shared metrics
    pub fn metrics(&self) -> Arc<GatewayMetrics> {
        Arc::clone(&self.metrics)
    }

    /// Get rate limit state for cleanup task
    pub fn rate_limit_state(&self) -> Arc<RateLimitState> {
        self.rate_limit.state()
    }

    /// Get circuit breaker manager
    pub fn circuit_breaker(&self) -> Arc<CircuitBreakerManager> {
        Arc::clone(&self.circuit_breaker)
    }
}

