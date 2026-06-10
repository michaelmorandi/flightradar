//! Auth endpoints: anonymous login, admin login, logout, me, refresh.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use flightradar_domain::ports::auth::TokenClaims;

use crate::dto::auth::{LoginRequest, LoginResponse, UserDto};
use crate::error::ApiError;
use crate::extractors::auth::AUTH_COOKIE;
use crate::extractors::Authenticated;
use crate::state::AppState;

fn build_cookie<'a>(token: String, ttl: Duration) -> Cookie<'a> {
    let secs = i64::try_from(ttl.as_secs()).unwrap_or(i64::MAX);
    Cookie::build((AUTH_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(true)
        .max_age(time::Duration::seconds(secs))
        .build()
}

fn build_removal_cookie<'a>() -> Cookie<'a> {
    // Without `path("/")` the removal cookie defaults to the request
    // path and won't actually clear the session cookie set at "/".
    Cookie::build((AUTH_COOKIE, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(true)
        .max_age(time::Duration::seconds(0))
        .build()
}

pub async fn anonymous(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
) -> Result<(PrivateCookieJar, Json<LoginResponse>), ApiError> {
    let outcome = state.auth.service.anonymous_login().await?;
    let ttl = ttl_until(outcome.expires_at);
    let cookie = build_cookie(outcome.token, ttl);
    let jar = jar.add(cookie);
    Ok((
        jar,
        Json(LoginResponse {
            user: outcome.user.into(),
            expires_at: outcome.expires_at,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(PrivateCookieJar, Json<LoginResponse>), ApiError> {
    let outcome = state
        .auth
        .service
        .admin_login(&req.email, &req.password)
        .await?;
    let ttl = ttl_until(outcome.expires_at);
    let cookie = build_cookie(outcome.token, ttl);
    let jar = jar.add(cookie);
    Ok((
        jar,
        Json(LoginResponse {
            user: outcome.user.into(),
            expires_at: outcome.expires_at,
        }),
    ))
}

pub async fn logout(jar: PrivateCookieJar) -> (PrivateCookieJar, StatusCode) {
    // The default `jar.remove(Cookie::from(name))` doesn't carry the
    // path; without `path("/")` browsers won't actually clear the
    // session cookie. Emit an explicit zero-age cookie at the correct
    // path so logout reliably terminates the session.
    let jar = jar.add(build_removal_cookie());
    (jar, StatusCode::NO_CONTENT)
}

pub async fn me(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Authenticated(claims): Authenticated,
) -> Result<(PrivateCookieJar, Json<UserDto>), ApiError> {
    // Sliding refresh: every authenticated call to /auth/me re-issues
    // the session cookie with a fresh TTL. This is the mechanism the
    // admin frontend leans on to keep its session alive past the JWT's
    // 15-minute hard expiry without holding the password in memory.
    let jar = refresh_session(&state, jar, &claims).await?;
    Ok((
        jar,
        Json(UserDto {
            id: claims.user_id.as_str().to_owned(),
            email: String::new(), // not in JWT claims; client should re-fetch if needed
            role: crate::dto::auth::role_str(claims.role).to_owned(),
            display_name: None,
            is_admin: claims.role == flightradar_domain::Role::Admin,
        }),
    ))
}

async fn refresh_session(
    state: &AppState,
    jar: PrivateCookieJar,
    claims: &TokenClaims,
) -> Result<PrivateCookieJar, ApiError> {
    let outcome = state
        .auth
        .service
        .issue_refresh(claims.user_id.clone(), claims.role)
        .await?;
    let ttl = ttl_until(outcome.expires_at);
    Ok(jar.add(build_cookie(outcome.token, ttl)))
}

fn ttl_until(expires_at: time::OffsetDateTime) -> Duration {
    let now = time::OffsetDateTime::now_utc();
    let diff = expires_at - now;
    if diff.is_positive() {
        let secs = u64::try_from(diff.whole_seconds()).unwrap_or(0);
        Duration::from_secs(secs)
    } else {
        Duration::from_secs(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[test]
    fn ttl_until_handles_past_expiry() {
        let past = OffsetDateTime::now_utc() - time::Duration::hours(1);
        assert_eq!(ttl_until(past), Duration::from_secs(0));
    }

    #[test]
    fn ttl_until_positive_for_future_expiry() {
        let future = OffsetDateTime::now_utc() + time::Duration::minutes(15);
        let ttl = ttl_until(future);
        assert!(ttl.as_secs() > 60 * 14);
        assert!(ttl.as_secs() <= 60 * 15);
    }

    #[test]
    fn cookie_has_expected_attributes() {
        let c = build_cookie("tok".into(), Duration::from_secs(900));
        assert_eq!(c.name(), AUTH_COOKIE);
        assert_eq!(c.value(), "tok");
        assert_eq!(c.http_only(), Some(true));
        assert_eq!(c.secure(), Some(true));
        assert_eq!(c.same_site(), Some(SameSite::Lax));
        assert_eq!(c.path(), Some("/"));
    }
}
