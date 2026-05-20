use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum DomainError {
    #[error("invalid ICAO24 address: {0:?}")]
    InvalidIcao24(String),

    #[error("invalid callsign: {0:?}")]
    InvalidCallsign(String),

    #[error("invalid airline ICAO designator: {0:?}")]
    InvalidAirlineIcao(String),

    #[error("invalid latitude: {0}")]
    InvalidLatitude(f64),

    #[error("invalid longitude: {0}")]
    InvalidLongitude(f64),

    #[error("empty value where one was required: {0}")]
    Empty(&'static str),
}
