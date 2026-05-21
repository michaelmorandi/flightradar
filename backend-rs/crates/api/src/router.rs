//! Top-level Axum router. Mounts every handler under `/api/v1` and
//! attaches the middleware stack.

use axum::routing::{get, post};
use axum::Router;

use crate::handlers;
use crate::middleware::{
    compression_layer, cors_layer, timeout_layer, trace_layer, MiddlewareConfig,
};
use crate::state::AppState;

pub fn build_router(state: AppState, middleware_config: &MiddlewareConfig) -> Router {
    let api = Router::new()
        // health + meta
        .route("/info", get(handlers::health::info))
        .route("/health/alive", get(handlers::health::alive))
        .route("/health/ready", get(handlers::health::ready))
        // auth
        .route("/auth/anonymous", post(handlers::auth::anonymous))
        .route("/auth/login", post(handlers::auth::login))
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/me", get(handlers::auth::me))
        // flights
        .route("/flights", get(handlers::flights::list))
        .route("/flights/:id", get(handlers::flights::get_one))
        .route("/flights/:id/positions", get(handlers::flights::history))
        // aircraft
        .route("/aircraft/:icao24", get(handlers::aircraft::get_one))
        .route("/aircraft", post(handlers::aircraft::get_many))
        // airlines
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
        );

    Router::new()
        .nest("/api/v1", api)
        .with_state(state)
        .layer(trace_layer())
        .layer(compression_layer())
        .layer(cors_layer(middleware_config))
        .layer(timeout_layer(middleware_config))
}
