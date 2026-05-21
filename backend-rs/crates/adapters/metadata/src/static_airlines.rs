//! In-memory `AirlineDirectory` backed by a JSON dictionary on disk.
//!
//! Wire shape matches the legacy `operators.json`:
//!
//! ```json
//! {
//!   "AFR": ["Air France", "France", "AIRFRANS", "AF"],
//!   "BAW": ["British Airways", "United Kingdom", "SPEEDBIRD", "BA"]
//! }
//! ```
//!
//! Each value is a 4-tuple `[name, country, callsign, iata]`. Empty
//! strings are coerced to `None`. The directory is loaded once at startup
//! and lives in memory for the process lifetime.

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;

use flightradar_domain::ports::airline_directory::{AirlineDirectory, AirlineDirectoryError};
use flightradar_domain::{Airline, AirlineIcao};

#[derive(Debug, Clone)]
pub struct StaticAirlineDirectory {
    airlines: Vec<Airline>,
    by_icao: HashMap<String, usize>,
}

impl StaticAirlineDirectory {
    pub fn empty() -> Self {
        Self {
            airlines: Vec::new(),
            by_icao: HashMap::new(),
        }
    }

    pub fn from_airlines(airlines: Vec<Airline>) -> Self {
        let by_icao = airlines
            .iter()
            .enumerate()
            .map(|(i, a)| (a.icao.to_string(), i))
            .collect();
        Self { airlines, by_icao }
    }

    /// Parse a JSON blob in the legacy `{ICAO: [name, country, cs, iata]}`
    /// shape. Entries with an invalid ICAO designator are silently
    /// dropped — the wire data has historically contained a few bad rows.
    pub fn from_json(body: &str) -> Result<Self, AirlineDirectoryError> {
        let map: HashMap<String, AirlineTuple> = serde_json::from_str(body)
            .map_err(|e| AirlineDirectoryError::Unavailable(format!("invalid JSON: {e}")))?;

        let airlines: Vec<Airline> = map
            .into_iter()
            .filter_map(|(icao, t)| build_airline(&icao, t))
            .collect();
        Ok(Self::from_airlines(airlines))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AirlineDirectoryError> {
        let body = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            AirlineDirectoryError::Unavailable(format!("cannot read airlines file: {e}"))
        })?;
        Self::from_json(&body)
    }

    pub fn len(&self) -> usize {
        self.airlines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.airlines.is_empty()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AirlineTuple {
    Quad([String; 4]),
    Triple([String; 3]),
    Double([String; 2]),
    Single([String; 1]),
}

fn build_airline(icao: &str, t: AirlineTuple) -> Option<Airline> {
    let parsed = AirlineIcao::new(icao).ok()?;
    let (name, country, callsign, iata) = match t {
        AirlineTuple::Quad([n, c, cs, i]) => (n, Some(c), Some(cs), Some(i)),
        AirlineTuple::Triple([n, c, cs]) => (n, Some(c), Some(cs), None),
        AirlineTuple::Double([n, c]) => (n, Some(c), None, None),
        AirlineTuple::Single([n]) => (n, None, None, None),
    };
    let name = name.trim().to_owned();
    if name.is_empty() {
        return None;
    }
    Some(Airline {
        icao: parsed,
        name,
        country: country.and_then(normalise),
        callsign: callsign.and_then(normalise),
        iata: iata.and_then(normalise),
    })
}

fn normalise(s: String) -> Option<String> {
    let trimmed_len = s.trim().len();
    if trimmed_len == 0 {
        return None;
    }
    if trimmed_len == s.len() {
        Some(s)
    } else {
        Some(s.trim().to_owned())
    }
}

#[async_trait]
impl AirlineDirectory for StaticAirlineDirectory {
    async fn find(&self, icao: &AirlineIcao) -> Result<Option<Airline>, AirlineDirectoryError> {
        Ok(self
            .by_icao
            .get(&icao.to_string())
            .and_then(|i| self.airlines.get(*i))
            .cloned())
    }

    async fn search(&self, query: &str, limit: u32) -> Result<Vec<Airline>, AirlineDirectoryError> {
        let needle = query.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let cap = usize::try_from(limit.max(1)).unwrap_or(usize::MAX);
        let mut out = Vec::with_capacity(cap.min(64));
        for a in &self.airlines {
            if a.icao.as_str().to_ascii_lowercase().contains(&needle)
                || a.name.to_ascii_lowercase().contains(&needle)
                || a.iata
                    .as_deref()
                    .is_some_and(|i| i.to_ascii_lowercase().contains(&needle))
            {
                out.push(a.clone());
                if out.len() >= cap {
                    break;
                }
            }
        }
        Ok(out)
    }

    async fn all(&self) -> Result<Vec<Airline>, AirlineDirectoryError> {
        Ok(self.airlines.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "AFR": ["Air France", "France", "AIRFRANS", "AF"],
        "BAW": ["British Airways", "United Kingdom", "SPEEDBIRD", "BA"],
        "DLH": ["Lufthansa", "Germany", "LUFTHANSA", "LH"],
        "AAA": ["Avicon", "Pakistan", "", "AN"],
        "BAD": ["Empty Name", "Nowhere", "", ""],
        "ZZ": ["Bad ICAO", "X", "", ""]
    }"#;

    #[tokio::test]
    async fn loads_typical_json() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        // ZZ is dropped (invalid 2-char ICAO). Everything else has a
        // non-empty `name`, so 5 of the 6 entries survive.
        let count = d.all().await.unwrap().len();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn entry_with_empty_name_field_is_dropped() {
        let body = r#"{"AAA":["   ", "Nowhere", "", ""]}"#;
        let d = StaticAirlineDirectory::from_json(body).unwrap();
        assert!(d.is_empty());
    }

    #[tokio::test]
    async fn find_returns_known_airline_with_fields() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        let a = d
            .find(&AirlineIcao::new("AFR").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a.name, "Air France");
        assert_eq!(a.country.as_deref(), Some("France"));
        assert_eq!(a.iata.as_deref(), Some("AF"));
    }

    #[tokio::test]
    async fn find_returns_none_for_missing_icao() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        let r = d.find(&AirlineIcao::new("XYZ").unwrap()).await.unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn empty_query_returns_no_matches() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        assert!(d.search("  ", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_matches_name_case_insensitive() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        let results = d.search("lufthansa", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].icao.as_str(), "DLH");
    }

    #[tokio::test]
    async fn search_matches_iata() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        let results = d.search("BA", 10).await.unwrap();
        // Matches "British Airways" via name OR iata "BA".
        assert!(results.iter().any(|a| a.icao.as_str() == "BAW"));
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let d = StaticAirlineDirectory::from_json(SAMPLE).unwrap();
        // "a" matches everything (broad).
        let results = d.search("a", 2).await.unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn entry_with_empty_iata_field_is_normalised_to_none() {
        let d =
            StaticAirlineDirectory::from_json(r#"{"AAA":["Avicon","Pakistan","","AN"]}"#).unwrap();
        let a = d
            .find(&AirlineIcao::new("AAA").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(a.callsign.is_none()); // "" → None
        assert_eq!(a.iata.as_deref(), Some("AN"));
    }

    #[tokio::test]
    async fn empty_directory_works() {
        let d = StaticAirlineDirectory::empty();
        assert!(d.is_empty());
        assert!(d.all().await.unwrap().is_empty());
        assert!(d.search("anything", 10).await.unwrap().is_empty());
    }

    #[test]
    fn malformed_json_returns_unavailable() {
        let err = StaticAirlineDirectory::from_json("not json").unwrap_err();
        assert!(matches!(err, AirlineDirectoryError::Unavailable(_)));
    }
}
