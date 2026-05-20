//! Auth extractors.
//!
//! [`Authenticated`] succeeds for any valid signed cookie; [`AdminUser`]
//! additionally enforces the `admin` role. Both return `ApiError` on
//! failure so handlers don't need to handle absent/bad tokens.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::PrivateCookieJar;

use flightradar_domain::ports::auth::TokenClaims;
use flightradar_domain::Role;

use crate::error::ApiError;
use crate::state::{AppState, AuthState};

/// Cookie name. Same value across all auth extractors and the login
/// handler. Server-side encrypted (PrivateCookieJar).
pub const AUTH_COOKIE: &str = "fr_session";

/// Authenticated identity extracted from the signed session cookie. Wraps
/// the JWT claims so handlers can act on user id and role.
#[derive(Debug, Clone)]
pub struct Authenticated(pub TokenClaims);

#[axum::async_trait]
impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = read_claims(parts, &state.auth)?;
        Ok(Authenticated(claims))
    }
}

/// Admin-only identity. Extraction fails with 403 if the user is
/// authenticated but not an admin, or 401 if not authenticated at all.
#[derive(Debug, Clone)]
pub struct AdminUser(pub TokenClaims);

#[axum::async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let claims = read_claims(parts, &state.auth)?;
        if claims.role != Role::Admin {
            return Err(ApiError::Forbidden);
        }
        Ok(AdminUser(claims))
    }
}

fn read_claims(parts: &mut Parts, auth: &AuthState) -> Result<TokenClaims, ApiError> {
    let jar = PrivateCookieJar::from_headers(&parts.headers, auth.cookie_key.clone());
    let cookie = jar.get(AUTH_COOKIE).ok_or(ApiError::Unauthenticated)?;
    auth.verifier
        .verify(cookie.value())
        .map_err(|_| ApiError::Unauthenticated)
}
