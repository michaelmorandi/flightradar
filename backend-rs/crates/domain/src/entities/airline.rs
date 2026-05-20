use serde::{Deserialize, Serialize};

use crate::value_objects::AirlineIcao;

/// Airline reference data, sourced from a static directory (today:
/// `operators.json`). Read-only at runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Airline {
    pub icao: AirlineIcao,
    pub name: String,
    pub country: Option<String>,
    pub callsign: Option<String>,
    pub iata: Option<String>,
}

impl Airline {
    pub fn new(icao: AirlineIcao, name: impl Into<String>) -> Self {
        Self {
            icao,
            name: name.into(),
            country: None,
            callsign: None,
            iata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_with_minimum_fields() {
        let a = Airline::new(AirlineIcao::new("AFR").unwrap(), "Air France");
        assert_eq!(a.icao.as_str(), "AFR");
        assert_eq!(a.name, "Air France");
        assert!(a.country.is_none());
        assert!(a.callsign.is_none());
        assert!(a.iata.is_none());
    }
}
