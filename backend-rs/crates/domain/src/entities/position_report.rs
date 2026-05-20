use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::DomainError;
use crate::value_objects::{Callsign, Icao24};

/// ADS-B aircraft category (DO-260B Table 2-67). Numeric values match the
/// `adsb.proto` enum used by the upstream gRPC stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum AircraftCategory {
    Unknown,
    NoInfo,
    Light,
    Medium1,
    Medium2,
    HighVortexLarge,
    Heavy,
    HighPerformance,
    Rotorcraft,
    Glider,
    LighterThanAir,
    Parachutist,
    Ultralight,
    Uav,
    Space,
    SurfaceEmergency,
    SurfaceService,
    PointObstacle,
    ClusterObstacle,
    LineObstacle,
    Reserved,
}

impl AircraftCategory {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<AircraftCategory> for u8 {
    fn from(c: AircraftCategory) -> Self {
        c.as_u8()
    }
}

impl TryFrom<u8> for AircraftCategory {
    type Error = DomainError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => AircraftCategory::Unknown,
            1 => AircraftCategory::NoInfo,
            2 => AircraftCategory::Light,
            3 => AircraftCategory::Medium1,
            4 => AircraftCategory::Medium2,
            5 => AircraftCategory::HighVortexLarge,
            6 => AircraftCategory::Heavy,
            7 => AircraftCategory::HighPerformance,
            8 => AircraftCategory::Rotorcraft,
            9 => AircraftCategory::Glider,
            10 => AircraftCategory::LighterThanAir,
            11 => AircraftCategory::Parachutist,
            12 => AircraftCategory::Ultralight,
            13 => AircraftCategory::Uav,
            14 => AircraftCategory::Space,
            15 => AircraftCategory::SurfaceEmergency,
            16 => AircraftCategory::SurfaceService,
            17 => AircraftCategory::PointObstacle,
            18 => AircraftCategory::ClusterObstacle,
            19 => AircraftCategory::LineObstacle,
            20 => AircraftCategory::Reserved,
            other => return Err(DomainError::Empty(category_out_of_range(other))),
        })
    }
}

fn category_out_of_range(_v: u8) -> &'static str {
    // Static message; the discriminant is logged separately by callers.
    "aircraft category out of range"
}

/// A single position observation from a radar source. Read-only data —
/// constructed by adapters, consumed by the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionReport {
    pub icao24: Icao24,
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_ft: Option<i32>,
    pub ground_speed_kt: Option<f64>,
    pub track_deg: Option<f64>,
    pub callsign: Option<Callsign>,
    pub category: Option<AircraftCategory>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl PositionReport {
    pub fn new(
        icao24: Icao24,
        latitude: f64,
        longitude: f64,
        observed_at: OffsetDateTime,
    ) -> Result<Self, DomainError> {
        if !(-90.0..=90.0).contains(&latitude) {
            return Err(DomainError::InvalidLatitude(latitude));
        }
        if !(-180.0..=180.0).contains(&longitude) {
            return Err(DomainError::InvalidLongitude(longitude));
        }
        Ok(Self {
            icao24,
            latitude,
            longitude,
            altitude_ft: None,
            ground_speed_kt: None,
            track_deg: None,
            callsign: None,
            category: None,
            observed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    #[test]
    fn rejects_out_of_range_latitude() {
        let err =
            PositionReport::new(Icao24::new("ABCDEF").unwrap(), 91.0, 0.0, now()).unwrap_err();
        assert!(matches!(err, DomainError::InvalidLatitude(_)));
    }

    #[test]
    fn rejects_out_of_range_longitude() {
        let err =
            PositionReport::new(Icao24::new("ABCDEF").unwrap(), 0.0, 181.0, now()).unwrap_err();
        assert!(matches!(err, DomainError::InvalidLongitude(_)));
    }

    #[test]
    fn category_roundtrip() {
        for v in 0_u8..=20 {
            let c = AircraftCategory::try_from(v).unwrap();
            assert_eq!(c.as_u8(), v);
        }
        assert!(AircraftCategory::try_from(21).is_err());
    }
}
