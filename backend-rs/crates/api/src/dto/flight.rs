//! Flight + position DTOs.

use serde::Serialize;
use time::OffsetDateTime;

use flightradar_domain::{Flight, PositionReport};

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct FlightDto {
    pub id: String,
    pub icao24: String,
    pub callsign: Option<String>,
    pub airline_icao: Option<String>,
    pub is_military: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub first_contact: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_contact: OffsetDateTime,
    pub duration_seconds: i64,
}

impl From<Flight> for FlightDto {
    fn from(f: Flight) -> Self {
        let duration_seconds = f.duration_seconds();
        Self {
            id: f.id.as_str().to_owned(),
            icao24: f.icao24.to_string(),
            callsign: f.callsign.map(|c| c.as_str().to_owned()),
            airline_icao: f.airline_icao.map(|a| a.as_str().to_owned()),
            is_military: f.is_military,
            first_contact: f.first_contact,
            last_contact: f.last_contact,
            duration_seconds,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct PositionDto {
    pub icao24: String,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: Option<i32>,
    pub ground_speed_kt: Option<f64>,
    pub track_deg: Option<f64>,
    pub callsign: Option<String>,
    pub category: Option<u8>,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

impl From<PositionReport> for PositionDto {
    fn from(p: PositionReport) -> Self {
        Self {
            icao24: p.icao24.to_string(),
            lat: p.latitude,
            lon: p.longitude,
            alt_ft: p.altitude_ft,
            ground_speed_kt: p.ground_speed_kt,
            track_deg: p.track_deg,
            callsign: p.callsign.map(|c| c.as_str().to_owned()),
            category: p.category.map(flightradar_domain::AircraftCategory::as_u8),
            observed_at: p.observed_at,
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    use flightradar_domain::{AirlineIcao, Callsign, FlightId, Icao24};

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    #[test]
    fn flight_dto_maps_all_fields() {
        let f = Flight {
            id: FlightId::new("f-1"),
            icao24: Icao24::new("ABCDEF").unwrap(),
            callsign: Some(Callsign::new("AFR990").unwrap()),
            airline_icao: Some(AirlineIcao::new("AFR").unwrap()),
            is_military: false,
            first_contact: t(0),
            last_contact: t(60),
        };
        let dto: FlightDto = f.into();
        assert_eq!(dto.id, "f-1");
        assert_eq!(dto.icao24, "ABCDEF");
        assert_eq!(dto.callsign.as_deref(), Some("AFR990"));
        assert_eq!(dto.airline_icao.as_deref(), Some("AFR"));
        assert_eq!(dto.duration_seconds, 60);
    }

    #[test]
    fn position_dto_maps_all_fields() {
        let mut pr = PositionReport::new(Icao24::new("ABCDEF").unwrap(), 47.0, 8.0, t(0)).unwrap();
        pr.altitude_ft = Some(30_000);
        pr.ground_speed_kt = Some(420.0);
        pr.track_deg = Some(180.0);
        pr.category = Some(flightradar_domain::AircraftCategory::Medium1);
        let dto: PositionDto = pr.into();
        assert_eq!(dto.icao24, "ABCDEF");
        assert_eq!(dto.lat, 47.0);
        assert_eq!(dto.alt_ft, Some(30_000));
        assert_eq!(dto.category, Some(3));
    }

    #[test]
    fn flight_dto_handles_none_callsign() {
        let f = Flight {
            id: FlightId::new("f-1"),
            icao24: Icao24::new("ABCDEF").unwrap(),
            callsign: None,
            airline_icao: None,
            is_military: true,
            first_contact: t(0),
            last_contact: t(0),
        };
        let dto: FlightDto = f.into();
        assert!(dto.callsign.is_none());
        assert!(dto.airline_icao.is_none());
        assert!(dto.is_military);
        assert_eq!(dto.duration_seconds, 0);
    }
}
