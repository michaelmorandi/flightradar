//! Callsign parsing — derives the ICAO airline designator from an ADS-B
//! callsign, or returns `None` for general-aviation / privacy / unrecognised
//! patterns.

use crate::value_objects::{AirlineIcao, Callsign};

const PRIVACY_PREFIXES: &[&str] = &["DCM", "FFL", "FWR", "XAA"];

/// Returns the 3-letter ICAO airline designator if the callsign matches a
/// commercial airline pattern, `None` otherwise.
pub fn extract_airline_icao(callsign: &Callsign) -> Option<AirlineIcao> {
    let cs = callsign.as_str();
    if cs.len() < 4 {
        return None;
    }
    if looks_like_ga_registration(cs) {
        return None;
    }
    let prefix = &cs[..3];
    if PRIVACY_PREFIXES.contains(&prefix) {
        return None;
    }
    let alphabetic_prefix = prefix.bytes().all(|b| b.is_ascii_alphabetic());
    let suffix_has_digit = cs[3..].bytes().any(|b| b.is_ascii_digit());
    if alphabetic_prefix && suffix_has_digit {
        AirlineIcao::new(prefix).ok()
    } else {
        None
    }
}

fn looks_like_ga_registration(cs: &str) -> bool {
    let bytes = cs.as_bytes();

    // Country prefix with dash: "G-ABCD", "D-EABC", "HB-JCS"
    if bytes.iter().take(3).any(|&b| b == b'-') {
        return true;
    }

    // US N-numbers: "N172SP"
    if bytes[0] == b'N' && bytes.get(1).is_some_and(u8::is_ascii_digit) {
        return true;
    }

    // Japan: "JA" followed by a digit
    if bytes[0] == b'J'
        && bytes.get(1) == Some(&b'A')
        && bytes.get(2).is_some_and(u8::is_ascii_digit)
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cs(s: &str) -> Callsign {
        Callsign::new(s).unwrap()
    }

    #[test]
    fn extracts_commercial_airline() {
        assert_eq!(extract_airline_icao(&cs("AFR990")).unwrap().as_str(), "AFR");
        assert_eq!(extract_airline_icao(&cs("BAW238")).unwrap().as_str(), "BAW");
        assert_eq!(extract_airline_icao(&cs("DLH4U")).unwrap().as_str(), "DLH");
    }

    #[test]
    fn rejects_general_aviation() {
        assert!(extract_airline_icao(&cs("N172SP")).is_none());
        assert!(extract_airline_icao(&cs("G-ABCD")).is_none());
        assert!(extract_airline_icao(&cs("HB-JCS")).is_none());
        assert!(extract_airline_icao(&cs("JA8089")).is_none());
    }

    #[test]
    fn rejects_privacy_prefixes() {
        assert!(extract_airline_icao(&cs("DCM1234")).is_none());
        assert!(extract_airline_icao(&cs("XAA9999")).is_none());
    }

    #[test]
    fn rejects_too_short() {
        assert!(extract_airline_icao(&cs("ABC")).is_none());
    }

    #[test]
    fn rejects_no_digit_suffix() {
        assert!(extract_airline_icao(&cs("ABCDEF")).is_none());
    }
}
