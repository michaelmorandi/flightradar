use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::value_objects::{AirlineIcao, Callsign, Icao24};

/// Stable identifier for a `Flight` aggregate. The value type is opaque to
/// callers; adapters map it to their native storage key (e.g. Mongo `ObjectId`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FlightId(String);

impl FlightId {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A flight aggregate: one logical trip of an aircraft, bounded by first
/// and last position contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flight {
    pub id: FlightId,
    pub icao24: Icao24,
    pub callsign: Option<Callsign>,
    pub airline_icao: Option<AirlineIcao>,
    pub is_military: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub first_contact: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_contact: OffsetDateTime,
}

impl Flight {
    pub fn duration_seconds(&self) -> i64 {
        (self.last_contact - self.first_contact).whole_seconds()
    }
}
