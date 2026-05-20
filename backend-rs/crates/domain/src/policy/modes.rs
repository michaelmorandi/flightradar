//! Mode-S / ICAO24 address classification.

use std::ops::RangeInclusive;

use crate::value_objects::Icao24;

/// Classifies ICAO24 addresses against pre-loaded military allocation ranges.
///
/// Range data is sourced externally (today: `mil_ranges.json` from
/// <https://github.com/wiedehopf/tar1090-db>). This struct owns no I/O; an
/// adapter loads the ranges and passes them in.
#[derive(Debug, Clone, Default)]
pub struct ModeSClassifier {
    military_ranges: Vec<RangeInclusive<u32>>,
}

impl ModeSClassifier {
    pub fn new(ranges: Vec<RangeInclusive<u32>>) -> Self {
        Self {
            military_ranges: ranges,
        }
    }

    pub fn from_hex_pairs<I, S>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (S, S)>,
        S: AsRef<str>,
    {
        let ranges = pairs
            .into_iter()
            .filter_map(|(lo, hi)| {
                let lo = u32::from_str_radix(lo.as_ref(), 16).ok()?;
                let hi = u32::from_str_radix(hi.as_ref(), 16).ok()?;
                Some(lo..=hi)
            })
            .collect();
        Self::new(ranges)
    }

    pub fn is_military(&self, addr: &Icao24) -> bool {
        let n = addr.as_u32();
        self.military_ranges.iter().any(|r| r.contains(&n))
    }

    /// Swiss military allocation (0x4B7000–0x4B7FFF).
    pub fn is_swiss_military(addr: &Icao24) -> bool {
        let n = addr.as_u32();
        (0x004B_7000..=0x004B_7FFF).contains(&n)
    }

    /// Swiss civilian + military (block 4B0xxx–4B8xxx).
    pub fn is_swiss(addr: &Icao24) -> bool {
        let bytes = addr.as_str().as_bytes();
        if bytes[0] != b'4' || bytes[1] != b'B' {
            return false;
        }
        let third = (bytes[2] as char).to_digit(16).unwrap_or(0xFF);
        third <= 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classifier() -> ModeSClassifier {
        ModeSClassifier::from_hex_pairs(vec![
            ("4B7000", "4B7FFF"), // Swiss military
            ("AE0000", "AFFFFF"), // US military
        ])
    }

    #[test]
    fn detects_military() {
        let c = classifier();
        assert!(c.is_military(&Icao24::new("4B7123").unwrap()));
        assert!(c.is_military(&Icao24::new("AE0001").unwrap()));
        assert!(!c.is_military(&Icao24::new("4B0123").unwrap()));
    }

    #[test]
    fn swiss_military_range() {
        assert!(ModeSClassifier::is_swiss_military(
            &Icao24::new("4B7000").unwrap()
        ));
        assert!(ModeSClassifier::is_swiss_military(
            &Icao24::new("4B7FFF").unwrap()
        ));
        assert!(!ModeSClassifier::is_swiss_military(
            &Icao24::new("4B8000").unwrap()
        ));
    }

    #[test]
    fn swiss_civil() {
        assert!(ModeSClassifier::is_swiss(&Icao24::new("4B0123").unwrap()));
        assert!(ModeSClassifier::is_swiss(&Icao24::new("4B8FFF").unwrap()));
        assert!(!ModeSClassifier::is_swiss(&Icao24::new("4B9000").unwrap()));
        assert!(!ModeSClassifier::is_swiss(&Icao24::new("4C0000").unwrap()));
    }

    #[test]
    fn from_hex_pairs_skips_invalid() {
        let c = ModeSClassifier::from_hex_pairs(vec![("ZZZZZZ", "FFFFFF"), ("AE0000", "AFFFFF")]);
        assert!(c.is_military(&Icao24::new("AE0001").unwrap()));
    }
}
