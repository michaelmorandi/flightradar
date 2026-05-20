//! Error helpers — translate Mongo / BSON failures into domain
//! `RepositoryError` so use cases never see crate-specific types.

use mongodb::error::{Error as MongoError, ErrorKind};
use thiserror::Error;

use flightradar_domain::ports::repositories::RepositoryError;

/// Errors that can occur while building or initialising a Mongo connection.
#[derive(Debug, Error)]
pub enum CodecError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    #[error("field {0:?} has unexpected type")]
    TypeMismatch(&'static str),

    #[error("invalid value for {0}: {1}")]
    InvalidValue(&'static str, String),
}

impl From<CodecError> for RepositoryError {
    fn from(err: CodecError) -> Self {
        RepositoryError::Backend(Box::new(err))
    }
}

pub fn map_mongo_error(err: MongoError) -> RepositoryError {
    if is_duplicate_key(&err) {
        return RepositoryError::Conflict("duplicate key".to_string());
    }
    if is_connection_error(&err) {
        return RepositoryError::Unavailable;
    }
    RepositoryError::Backend(Box::new(err))
}

fn is_duplicate_key(err: &MongoError) -> bool {
    // Duplicate-key error is identified by Mongo error code 11000, which
    // ends up in the formatted error string for both Write and Command kinds.
    err.to_string().contains("E11000") || err.to_string().contains("11000")
}

fn is_connection_error(err: &MongoError) -> bool {
    matches!(
        &*err.kind,
        ErrorKind::Io(_) | ErrorKind::ServerSelection { .. } | ErrorKind::DnsResolve { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_error_wraps_into_repository_backend() {
        let err = RepositoryError::from(CodecError::MissingField("icao24"));
        assert!(matches!(err, RepositoryError::Backend(_)));
        assert!(err.to_string().contains("database error"));
    }

    #[test]
    fn codec_error_message_includes_field() {
        let e = CodecError::MissingField("callsign");
        assert!(e.to_string().contains("callsign"));
        let e = CodecError::InvalidValue("lat", "x".into());
        assert!(e.to_string().contains("lat"));
        assert!(e.to_string().contains('x'));
    }
}
