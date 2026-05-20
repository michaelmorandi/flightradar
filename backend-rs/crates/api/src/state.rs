//! Shared application state passed to every handler.

use std::sync::Arc;

use axum_extra::extract::cookie::Key;

use flightradar_application::{
    AircraftQuery, AirlineQuery, AuthService, FlightQuery, TokioBroadcastBus,
};
use flightradar_domain::ports::auth::TokenVerifier;

#[derive(Clone, Debug)]
pub struct BuildInfo {
    pub commit: String,
    pub build_timestamp: String,
}

/// Auth side of the state. Split from `AppState` so it can implement
/// `FromRef<AppState>` and be used as the `S` parameter on
/// `axum_extra::extract::cookie::PrivateCookieJar`.
#[derive(Clone)]
pub struct AuthState {
    pub service: Arc<AuthService>,
    pub verifier: Arc<dyn TokenVerifier>,
    pub cookie_key: Key,
}

impl std::fmt::Debug for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthState").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub flights: Arc<FlightQuery>,
    pub aircraft: Arc<AircraftQuery>,
    pub airlines: Arc<AirlineQuery>,
    pub auth: AuthState,
    pub events: Arc<TokioBroadcastBus>,
    pub build: BuildInfo,
}

// Required by axum_extra's PrivateCookieJar.
impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.auth.cookie_key.clone()
    }
}

impl axum::extract::FromRef<AppState> for AuthState {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}
