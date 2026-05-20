//! Live in-memory state of all currently tracked aircraft.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::entities::position_report::AircraftCategory;
use crate::value_objects::{Callsign, Icao24};

/// Lightweight position view broadcast to live subscribers. Keyed by
/// `Icao24` — the natural ADS-B identifier and what the wire format exposes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivePosition {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: Option<i32>,
    pub ground_speed_kt: Option<f64>,
    pub track_deg: Option<f64>,
    pub callsign: Option<Callsign>,
    pub category: Option<AircraftCategory>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl LivePosition {
    /// Two live positions are "meaningfully different" if any field that a
    /// subscriber renders has changed. Used to decide whether to emit a
    /// delta entry on a tick.
    ///
    /// Exact float comparison is intentional: lat/lon are copied through
    /// from the source unchanged, so we detect any difference at all
    /// (including the bit-identical case → no change).
    #[allow(clippy::float_cmp)]
    pub fn differs_from(&self, other: &Self) -> bool {
        self.lat != other.lat
            || self.lon != other.lon
            || self.alt_ft != other.alt_ft
            || self.ground_speed_kt != other.ground_speed_kt
            || self.track_deg != other.track_deg
            || self.callsign != other.callsign
            || self.category != other.category
    }
}

/// Immutable snapshot of the entire live picture. Produced by the flight
/// updater, consumed (cheaply, lock-free) by HTTP and SSE handlers.
#[derive(Debug, Clone, Default)]
pub struct LiveSnapshot {
    positions: HashMap<Icao24, LivePosition>,
    generated_at: Option<OffsetDateTime>,
}

impl LiveSnapshot {
    pub fn new(positions: HashMap<Icao24, LivePosition>, generated_at: OffsetDateTime) -> Self {
        Self {
            positions,
            generated_at: Some(generated_at),
        }
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn positions(&self) -> &HashMap<Icao24, LivePosition> {
        &self.positions
    }

    pub fn get(&self, icao24: &Icao24) -> Option<&LivePosition> {
        self.positions.get(icao24)
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn generated_at(&self) -> Option<OffsetDateTime> {
        self.generated_at
    }

    /// Return the entries in `self` that are new or changed compared with
    /// `previous`. Removed aircraft are signalled separately (see
    /// [`removed_since`](Self::removed_since)).
    pub fn delta_against(&self, previous: &Self) -> HashMap<Icao24, LivePosition> {
        let mut out = HashMap::new();
        for (icao, current) in &self.positions {
            match previous.positions.get(icao) {
                Some(prev) if !current.differs_from(prev) => {}
                _ => {
                    out.insert(icao.clone(), current.clone());
                }
            }
        }
        out
    }

    /// ICAOs that were in `previous` but are not in `self`.
    pub fn removed_since(&self, previous: &Self) -> Vec<Icao24> {
        previous
            .positions
            .keys()
            .filter(|k| !self.positions.contains_key(*k))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn lp(lat: f64, lon: f64) -> LivePosition {
        LivePosition {
            lat,
            lon,
            alt_ft: None,
            ground_speed_kt: None,
            track_deg: None,
            callsign: None,
            category: None,
            updated_at: now(),
        }
    }

    fn snap(entries: &[(&str, LivePosition)]) -> LiveSnapshot {
        let map = entries
            .iter()
            .map(|(hex, p)| (Icao24::new(hex).unwrap(), p.clone()))
            .collect();
        LiveSnapshot::new(map, now())
    }

    #[test]
    fn empty_snapshot_has_no_positions() {
        let s = LiveSnapshot::empty();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert!(s.generated_at().is_none());
    }

    #[test]
    fn get_returns_known_position() {
        let s = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        let icao = Icao24::new("ABCDEF").unwrap();
        assert!(s.get(&icao).is_some());
        assert!(s.get(&Icao24::new("000001").unwrap()).is_none());
    }

    #[test]
    fn delta_includes_new_entries() {
        let prev = LiveSnapshot::empty();
        let curr = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        assert_eq!(curr.delta_against(&prev).len(), 1);
    }

    #[test]
    fn delta_excludes_unchanged_entries() {
        let prev = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        let curr = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        assert!(curr.delta_against(&prev).is_empty());
    }

    #[test]
    fn delta_includes_changed_entries() {
        let prev = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        let curr = snap(&[("ABCDEF", lp(1.0, 3.0))]);
        let d = curr.delta_against(&prev);
        assert_eq!(d.len(), 1);
        assert_eq!(d.get(&Icao24::new("ABCDEF").unwrap()).unwrap().lon, 3.0);
    }

    #[test]
    fn removed_lists_dropped_icaos() {
        let prev = snap(&[("ABCDEF", lp(1.0, 2.0)), ("123456", lp(5.0, 6.0))]);
        let curr = snap(&[("ABCDEF", lp(1.0, 2.0))]);
        let r = curr.removed_since(&prev);
        assert_eq!(r, vec![Icao24::new("123456").unwrap()]);
    }

    #[test]
    fn differs_from_detects_changes_per_field() {
        let base = lp(1.0, 2.0);
        let mut other = base.clone();
        assert!(!base.differs_from(&other));
        other.lat = 9.0;
        assert!(base.differs_from(&other));
        let mut other = base.clone();
        other.callsign = Some(Callsign::new("AFR990").unwrap());
        assert!(base.differs_from(&other));
        let mut other = base.clone();
        other.alt_ft = Some(30_000);
        assert!(base.differs_from(&other));
    }
}
