//! Auth endpoints: anonymous login, admin login, logout, me.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};

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
    let jar = jar.remove(Cookie::from(AUTH_COOKIE));
    (jar, StatusCode::NO_CONTENT)
}

pub async fn me(Authenticated(claims): Authenticated) -> Json<UserDto> {
    Json(UserDto {
        id: claims.user_id.as_str().to_owned(),
        email: String::new(), // not in JWT claims; client should re-fetch if needed
        role: crate::dto::auth::role_str(claims.role).to_owned(),
        display_name: None,
        is_admin: claims.role == flightradar_domain::Role::Admin,
    })
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
