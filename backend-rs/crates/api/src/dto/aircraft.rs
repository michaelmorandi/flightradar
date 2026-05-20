//! Aircraft DTOs.

use serde::{Deserialize, Serialize};

use flightradar_domain::Aircraft;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AircraftDto {
    pub icao24: String,
    pub registration: Option<String>,
    pub type_code: Option<String>,
    pub type_description: Option<String>,
    pub operator: Option<String>,
    pub designator: Option<String>,
    pub source: Option<String>,
}

impl From<Aircraft> for AircraftDto {
    fn from(a: Aircraft) -> Self {
        Self {
            icao24: a.icao24.to_string(),
            registration: a.registration,
            type_code: a.type_code,
            type_description: a.type_description,
            operator: a.operator,
            designator: a.designator,
            source: a.source.map(|s| s.as_str().to_owned()),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BulkAircraftRequest {
    pub icao24s: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkAircraftResponse {
    pub aircraft: Vec<AircraftDto>,
    pub requested: usize,
    pub found: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use flightradar_domain::{AircraftSource, Icao24};

    #[test]
    fn aircraft_dto_round_trips() {
        let mut ac = Aircraft::new(Icao24::new("ABCDEF").unwrap());
        ac.registration = Some("HB-JCS".into());
        ac.type_code = Some("A320".into());
        ac.operator = Some("Swiss".into());
        ac.source = Some(AircraftSource::new("nighthawk"));
        let dto: AircraftDto = ac.into();
        assert_eq!(dto.icao24, "ABCDEF");
        assert_eq!(dto.operator.as_deref(), Some("Swiss"));
        assert_eq!(dto.source.as_deref(), Some("nighthawk"));
    }

    #[test]
    fn aircraft_dto_serialises_camel_unaffected() {
        let mut ac = Aircraft::new(Icao24::new("ABCDEF").unwrap());
        ac.registration = Some("X".into());
        let dto: AircraftDto = ac.into();
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains(r#""icao24":"ABCDEF""#));
        assert!(json.contains(r#""registration":"X""#));
    }
}
