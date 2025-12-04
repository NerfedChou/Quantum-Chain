//! CORS middleware per SPEC-16 Section 7.5.
//!
//! Wrapper around tower-http CORS with gateway configuration.

use crate::domain::config::CorsConfig;
use axum::http::{HeaderName, Method};
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer as TowerCorsLayer};

/// Create CORS layer from gateway config
pub fn create_cors_layer(config: &CorsConfig) -> TowerCorsLayer {
    if !config.enabled {
        // Return permissive CORS that effectively disables it
        return TowerCorsLayer::very_permissive();
    }

    let mut cors = TowerCorsLayer::new();

    // Configure origins
    if config.allowed_origins.contains(&"*".to_string()) {
        cors = cors.allow_origin(Any);
    } else {
        let origins: Vec<_> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors = cors.allow_origin(origins);
    }

    // Configure methods
    let methods: Vec<Method> = config
        .allowed_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();
    cors = cors.allow_methods(methods);

    // Configure headers
    if config.allowed_headers.contains(&"*".to_string()) {
        cors = cors.allow_headers(Any);
    } else {
        let headers: Vec<HeaderName> = config
            .allowed_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        cors = cors.allow_headers(headers);
    }

    // Configure expose headers
    if !config.expose_headers.is_empty() {
        let expose: Vec<HeaderName> = config
            .expose_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        cors = cors.expose_headers(expose);
    }

    // Max age
    cors = cors.max_age(Duration::from_secs(config.max_age));

    // Credentials
    if config.allow_credentials {
        cors = cors.allow_credentials(true);
    }

    cors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_cors_config() {
        let config = CorsConfig::default();
        let layer = create_cors_layer(&config);
        // Just verify it creates without panic
        assert!(config.enabled);
    }

    #[test]
    fn test_disabled_cors() {
        let mut config = CorsConfig::default();
        config.enabled = false;
        let layer = create_cors_layer(&config);
        // Should create permissive layer
    }

    #[test]
    fn test_specific_origins() {
        let config = CorsConfig {
            enabled: true,
            allowed_origins: vec![
                "https://example.com".to_string(),
                "https://app.example.com".to_string(),
            ],
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
            allowed_headers: vec!["Content-Type".to_string()],
            expose_headers: vec![],
            max_age: 3600,
            allow_credentials: false,
        };
        let layer = create_cors_layer(&config);
        // Verify creation
    }
}
