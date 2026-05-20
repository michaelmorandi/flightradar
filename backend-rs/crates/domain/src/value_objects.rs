//! Value objects.
//!
//! Newtypes that enforce invariants at construction. Once you hold an
//! `Icao24`, it is guaranteed to be a 6-character uppercase hex string;
//! the rest of the code never has to revalidate.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;

// ---------------------------------------------------------------------------
// Icao24
// ---------------------------------------------------------------------------

/// 24-bit ICAO Mode-S address, rendered as 6 uppercase hex characters.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Icao24(String);

impl Icao24 {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim();
        if trimmed.len() != 6 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidIcao24(raw.to_owned()));
        }
        Ok(Self(trimmed.to_ascii_uppercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The address as a 24-bit unsigned integer.
    pub fn as_u32(&self) -> u32 {
        // Construction guarantees 6 hex chars, so parsing cannot fail.
        u32::from_str_radix(&self.0, 16).expect("Icao24 invariant violated")
    }
}

impl fmt::Display for Icao24 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Icao24 {
    type Err = DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl<'de> Deserialize<'de> for Icao24 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Callsign
// ---------------------------------------------------------------------------

/// ADS-B callsign — uppercase, trimmed, max 8 chars (ICAO Annex 10).
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Callsign(String);

impl Callsign {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim().to_ascii_uppercase();
        if trimmed.is_empty() || trimmed.len() > 8 {
            return Err(DomainError::InvalidCallsign(raw.to_owned()));
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(DomainError::InvalidCallsign(raw.to_owned()));
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Callsign {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Callsign {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// AirlineIcao
// ---------------------------------------------------------------------------

/// 3-letter ICAO airline designator (e.g. "AFR", "BAW", "DLH").
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct AirlineIcao(String);

impl AirlineIcao {
    pub fn new(raw: &str) -> Result<Self, DomainError> {
        let trimmed = raw.trim().to_ascii_uppercase();
        if trimmed.len() != 3 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(DomainError::InvalidAirlineIcao(raw.to_owned()));
        }
        Ok(Self(trimmed))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AirlineIcao {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for AirlineIcao {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icao24_normalises_to_uppercase() {
        let addr = Icao24::new("4b7123").unwrap();
        assert_eq!(addr.as_str(), "4B7123");
    }

    #[test]
    fn icao24_trims_whitespace() {
        let addr = Icao24::new("  4B7123  ").unwrap();
        assert_eq!(addr.as_str(), "4B7123");
    }

    #[test]
    fn icao24_rejects_wrong_length() {
        assert!(matches!(
            Icao24::new("4B712"),
            Err(DomainError::InvalidIcao24(_))
        ));
        assert!(matches!(
            Icao24::new("4B71234"),
            Err(DomainError::InvalidIcao24(_))
        ));
    }

    #[test]
    fn icao24_rejects_non_hex() {
        assert!(matches!(
            Icao24::new("4B71ZZ"),
            Err(DomainError::InvalidIcao24(_))
        ));
    }

    #[test]
    fn icao24_as_u32() {
        assert_eq!(Icao24::new("4B7123").unwrap().as_u32(), 0x004B_7123);
        assert_eq!(Icao24::new("FFFFFF").unwrap().as_u32(), 0x00FF_FFFF);
        assert_eq!(Icao24::new("000000").unwrap().as_u32(), 0);
    }

    #[test]
    fn callsign_accepts_typical() {
        assert!(Callsign::new("AFR990").is_ok());
        assert!(Callsign::new("N172SP").is_ok());
        assert!(Callsign::new("G-ABCD").is_ok());
    }

    #[test]
    fn callsign_rejects_empty_and_overlong() {
        assert!(Callsign::new("").is_err());
        assert!(Callsign::new("ABCDEFGHI").is_err());
    }

    #[test]
    fn airline_icao_must_be_three_letters() {
        assert_eq!(AirlineIcao::new("afr").unwrap().as_str(), "AFR");
        assert!(AirlineIcao::new("AFRX").is_err());
        assert!(AirlineIcao::new("AF").is_err());
        assert!(AirlineIcao::new("AF1").is_err());
    }
}
