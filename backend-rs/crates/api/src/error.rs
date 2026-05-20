//! HTTP error envelope.
//!
//! One type maps every `ApplicationError` to a status code + JSON body.
//! Handlers always return `Result<T, ApiError>`; mapping is automatic.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

use flightradar_application::ApplicationError;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthenticated")]
    Unauthenticated,

    #[error("forbidden")]
    Forbidden,

    #[error("rate limited")]
    RateLimited,

    #[error("upstream unavailable: {0}")]
    Unavailable(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthenticated => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden => StatusCode::FORBIDDEN,
            ApiError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            ApiError::NotFound => "not_found",
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthenticated => "unauthenticated",
            ApiError::Forbidden => "forbidden",
            ApiError::RateLimited => "rate_limited",
            ApiError::Unavailable(_) => "unavailable",
            ApiError::Internal(_) => "internal",
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = Json(ErrorBody {
            code: self.code(),
            message: self.to_string(),
        });
        (status, body).into_response()
    }
}

impl From<ApplicationError> for ApiError {
    fn from(err: ApplicationError) -> Self {
        use flightradar_domain::ports::auth::AuthError;
        use flightradar_domain::ports::metadata_source::MetadataError;
        use flightradar_domain::ports::radar_source::RadarError;
        use flightradar_domain::ports::repositories::RepositoryError;

        match err {
            ApplicationError::NotFound
            | ApplicationError::Repository(RepositoryError::NotFound) => ApiError::NotFound,
            ApplicationError::InvalidInput(m)
            | ApplicationError::Repository(RepositoryError::Conflict(m)) => ApiError::BadRequest(m),
            ApplicationError::Unauthenticated
            | ApplicationError::Auth(
                AuthError::Expired | AuthError::Malformed | AuthError::InvalidCredentials,
            ) => ApiError::Unauthenticated,
            ApplicationError::Forbidden => ApiError::Forbidden,
            ApplicationError::Domain(d) => ApiError::BadRequest(d.to_string()),
            ApplicationError::Auth(AuthError::Backend(e)) => ApiError::Internal(e.to_string()),
            ApplicationError::Metadata(MetadataError::RateLimited) => ApiError::RateLimited,
            ApplicationError::Metadata(MetadataError::Unavailable(m))
            | ApplicationError::Radar(RadarError::Unavailable(m)) => ApiError::Unavailable(m),
            ApplicationError::Metadata(_) => ApiError::Unavailable("metadata source error".into()),
            ApplicationError::Radar(_) => ApiError::Unavailable("radar source error".into()),
            ApplicationError::Repository(RepositoryError::Unavailable) => {
                ApiError::Unavailable("database unavailable".into())
            }
            ApplicationError::Repository(RepositoryError::Backend(e)) => {
                ApiError::Internal(e.to_string())
            }
            ApplicationError::AirlineDirectory(_) => {
                ApiError::Unavailable("airline directory error".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flightradar_domain::ports::auth::AuthError;
    use flightradar_domain::ports::metadata_source::MetadataError;
    use flightradar_domain::ports::repositories::RepositoryError;

    #[test]
    fn maps_not_found() {
        let e: ApiError = ApplicationError::NotFound.into();
        assert!(matches!(e, ApiError::NotFound));
        assert_eq!(e.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn maps_invalid_input_to_bad_request() {
        let e: ApiError = ApplicationError::InvalidInput("nope".into()).into();
        assert!(matches!(e, ApiError::BadRequest(_)));
    }

    #[test]
    fn maps_unauthenticated() {
        let e: ApiError = ApplicationError::Unauthenticated.into();
        assert_eq!(e.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn maps_expired_token_to_unauthenticated() {
        let e: ApiError = ApplicationError::Auth(AuthError::Expired).into();
        assert_eq!(e.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn maps_rate_limited_metadata() {
        let e: ApiError = ApplicationError::Metadata(MetadataError::RateLimited).into();
        assert_eq!(e.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn maps_repo_not_found() {
        let e: ApiError = ApplicationError::Repository(RepositoryError::NotFound).into();
        assert!(matches!(e, ApiError::NotFound));
    }

    #[test]
    fn maps_repo_unavailable_to_503() {
        let e: ApiError = ApplicationError::Repository(RepositoryError::Unavailable).into();
        assert_eq!(e.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(ApiError::NotFound.code(), "not_found");
        assert_eq!(ApiError::BadRequest("x".into()).code(), "bad_request");
        assert_eq!(ApiError::Unauthenticated.code(), "unauthenticated");
        assert_eq!(ApiError::Forbidden.code(), "forbidden");
        assert_eq!(ApiError::RateLimited.code(), "rate_limited");
        assert_eq!(ApiError::Unavailable("x".into()).code(), "unavailable");
        assert_eq!(ApiError::Internal("x".into()).code(), "internal");
    }
}
