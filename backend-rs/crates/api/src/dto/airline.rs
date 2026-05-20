//! Airline DTOs.

use serde::Serialize;

use flightradar_domain::Airline;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct AirlineDto {
    pub icao: String,
    pub name: String,
    pub country: Option<String>,
    pub callsign: Option<String>,
    pub iata: Option<String>,
}

impl From<Airline> for AirlineDto {
    fn from(a: Airline) -> Self {
        Self {
            icao: a.icao.to_string(),
            name: a.name,
            country: a.country,
            callsign: a.callsign,
            iata: a.iata,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flightradar_domain::AirlineIcao;

    #[test]
    fn airline_dto_maps_all_fields() {
        let mut a = Airline::new(AirlineIcao::new("AFR").unwrap(), "Air France");
        a.country = Some("France".into());
        a.iata = Some("AF".into());
        let dto: AirlineDto = a.into();
        assert_eq!(dto.icao, "AFR");
        assert_eq!(dto.name, "Air France");
        assert_eq!(dto.country.as_deref(), Some("France"));
        assert_eq!(dto.iata.as_deref(), Some("AF"));
    }
}
