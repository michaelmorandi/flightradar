//! Middleware stack helpers.
//!
//! The full Tower layer chain is assembled inside [`crate::router::build_router`].
//! These helpers expose individual layer builders so the server crate can
//! configure CORS origins, request timeout, etc. without having to depend
//! on tower-http directly.

use std::time::Duration;

use axum::http::{HeaderValue, Method};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

#[derive(Debug, Clone)]
pub struct MiddlewareConfig {
    pub allowed_origins: Vec<String>,
    pub request_timeout: Duration,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["http://localhost:5173".into()],
            request_timeout: Duration::from_secs(30),
        }
    }
}

pub fn cors_layer(config: &MiddlewareConfig) -> CorsLayer {
    let origins: Vec<HeaderValue> = config
        .allowed_origins
        .iter()
        .filter_map(|s| HeaderValue::from_str(s).ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_credentials(true)
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
        ])
}

pub fn timeout_layer(config: &MiddlewareConfig) -> TimeoutLayer {
    TimeoutLayer::with_status_code(
        axum::http::StatusCode::REQUEST_TIMEOUT,
        config.request_timeout,
    )
}

pub fn compression_layer() -> CompressionLayer {
    CompressionLayer::new()
}

pub fn trace_layer(
) -> TraceLayer<tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>>
{
    TraceLayer::new_for_http()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_are_local_friendly() {
        let cfg = MiddlewareConfig::default();
        assert_eq!(
            cfg.allowed_origins,
            vec!["http://localhost:5173".to_string()]
        );
        assert!(cfg.request_timeout >= Duration::from_secs(10));
    }

    #[test]
    fn invalid_origin_strings_are_dropped() {
        // Should not panic; the layer just won't list malformed origins.
        let cfg = MiddlewareConfig {
            allowed_origins: vec!["not-a-valid-header\n".into(), "https://ok".into()],
            ..MiddlewareConfig::default()
        };
        let _ = cors_layer(&cfg);
    }
}
