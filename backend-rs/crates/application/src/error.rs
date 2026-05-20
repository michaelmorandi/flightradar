use thiserror::Error;

use flightradar_domain::ports::airline_directory::AirlineDirectoryError;
use flightradar_domain::ports::auth::AuthError;
use flightradar_domain::ports::metadata_source::MetadataError;
use flightradar_domain::ports::radar_source::RadarError;
use flightradar_domain::ports::repositories::RepositoryError;

/// Top-level application error. Use cases bubble these out; the API layer
/// maps them to HTTP responses.
#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error("not found")]
    NotFound,

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unauthenticated")]
    Unauthenticated,

    #[error("forbidden")]
    Forbidden,

    #[error("radar source error: {0}")]
    Radar(#[from] RadarError),

    #[error("metadata source error: {0}")]
    Metadata(#[from] MetadataError),

    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),

    #[error("airline directory error: {0}")]
    AirlineDirectory(#[from] AirlineDirectoryError),

    #[error("auth error: {0}")]
    Auth(#[from] AuthError),

    #[error("domain error: {0}")]
    Domain(#[from] flightradar_domain::DomainError),
}

impl ApplicationError {
    /// Helper to convert a missing-row repository error into `NotFound`,
    /// keeping API mapping logic out of use cases.
    pub fn from_repo_lookup<T>(result: Result<Option<T>, RepositoryError>) -> Result<T, Self> {
        match result {
            Ok(Some(value)) => Ok(value),
            Ok(None) => Err(Self::NotFound),
            Err(err) => Err(err.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_repo_lookup_unwraps_some() {
        let r: Result<Option<i32>, RepositoryError> = Ok(Some(42));
        assert_eq!(ApplicationError::from_repo_lookup(r).unwrap(), 42);
    }

    #[test]
    fn from_repo_lookup_maps_none_to_not_found() {
        let r: Result<Option<i32>, RepositoryError> = Ok(None);
        let err = ApplicationError::from_repo_lookup(r).unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[test]
    fn from_repo_lookup_propagates_error() {
        let r: Result<Option<i32>, RepositoryError> = Err(RepositoryError::Unavailable);
        let err = ApplicationError::from_repo_lookup(r).unwrap_err();
        assert!(matches!(err, ApplicationError::Repository(_)));
    }
}
