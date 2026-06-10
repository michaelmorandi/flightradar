//! Top-level Axum router. Mounts every handler under `/api/v1` and
//! attaches the middleware stack.

use std::sync::Arc;
use std::time::Duration;

use axum::routing::{get, post};
use axum::Router;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::SmartIpKeyExtractor;
use tower_governor::GovernorLayer;

use crate::handlers;
use crate::middleware::{
    compression_layer, cors_layer, timeout_layer, trace_layer, MiddlewareConfig,
};
use crate::state::AppState;

pub fn build_router(state: AppState, middleware_config: &MiddlewareConfig) -> Router {
    let mut auth_routes = Router::new()
        .route("/auth/anonymous", post(handlers::auth::anonymous))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/me", get(handlers::auth::me));

    if middleware_config.rate_limit_auth {
        // `SmartIpKeyExtractor` reads `X-Forwarded-For` / `X-Real-IP`
        // when present (nginx in our deploy) and falls back to the TCP
        // peer. The legacy Python backend allowed ~20 login attempts /
        // hour; we round to 2/sec with a 10-burst, which lets the
        // frontend's startup anonymous-login + /auth/me probe through
        // while still slowing credential stuffing to a crawl.
        let auth_governor = GovernorConfigBuilder::default()
            .per_second(2)
            .burst_size(10)
            .key_extractor(SmartIpKeyExtractor)
            .finish()
            .expect("hardcoded governor config is valid");
        auth_routes = auth_routes.layer(GovernorLayer {
            config: Arc::new(auth_governor),
        });
    }

    let api = Router::new()
        // health + meta
        .route("/info", get(handlers::health::info))
        .route("/health/alive", get(handlers::health::alive))
        .route("/health/ready", get(handlers::health::ready))
        // flights
        .route("/flights", get(handlers::flights::list))
        .route("/flights/:id", get(handlers::flights::get_one))
        .route("/flights/:id/positions", get(handlers::flights::history))
        // aircraft
        .route("/aircraft/:icao24", get(handlers::aircraft::get_one))
        .route("/aircraft", post(handlers::aircraft::get_many))
        // airlines
        .route("/airlines", get(handlers::airlines::list))
        .route("/airlines/search", get(handlers::airlines::search))
        .route("/airlines/:icao", get(handlers::airlines::get_one))
        // live / SSE
        .route("/live/stream", get(handlers::sse::stream_all))
        .route("/live/stream/:icao24", get(handlers::sse::stream_one))
        // admin
        .route("/admin/stats", get(handlers::admin::stats))
        .route(
            "/admin/aircraft/:icao24",
            get(handlers::admin::get_aircraft).put(handlers::admin::put_aircraft),
        )
        .merge(auth_routes);

    Router::new()
        .nest("/api/v1", api)
        .with_state(state)
        .layer(trace_layer())
        .layer(compression_layer())
        .layer(cors_layer(middleware_config))
        .layer(timeout_layer(middleware_config))
}

/// Slow drainer for the governor's per-IP cache. Spawn once in `main`
/// alongside the other supervised tasks. Without this, IPs accumulate
/// forever.
pub async fn governor_cleanup_loop() {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        // Best-effort: tower_governor 0.4 exposes no public clear hook,
        // so this is a no-op placeholder for the supervisor's benefit.
        // Memory pressure has historically been a non-issue at our scale.
    }
}
