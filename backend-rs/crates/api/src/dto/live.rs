//! Live (SSE) DTOs.

use std::collections::HashMap;

use serde::Serialize;
use time::OffsetDateTime;

use flightradar_domain::ports::event_bus::PositionEvent;
use flightradar_domain::LivePosition;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct LivePositionDto {
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: Option<i32>,
    pub ground_speed_kt: Option<f64>,
    pub track_deg: Option<f64>,
    pub callsign: Option<String>,
    pub category: Option<u8>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<LivePosition> for LivePositionDto {
    fn from(p: LivePosition) -> Self {
        Self {
            lat: p.lat,
            lon: p.lon,
            alt_ft: p.alt_ft,
            ground_speed_kt: p.ground_speed_kt,
            track_deg: p.track_deg,
            callsign: p.callsign.map(|c| c.as_str().to_owned()),
            category: p.category.map(flightradar_domain::AircraftCategory::as_u8),
            updated_at: p.updated_at,
        }
    }
}

/// SSE wire-format snapshot. `data: { … }` for the `snapshot` event.
#[derive(Debug, Serialize)]
pub struct SnapshotPayload {
    pub positions: HashMap<String, LivePositionDto>,
    #[serde(with = "time::serde::rfc3339")]
    pub emitted_at: OffsetDateTime,
}

/// SSE wire-format delta. `data: { … }` for the `delta` event.
#[derive(Debug, Serialize)]
pub struct DeltaPayload {
    pub changed: HashMap<String, LivePositionDto>,
    pub removed: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub emitted_at: OffsetDateTime,
}

#[derive(Debug)]
pub enum LiveEnvelope {
    Snapshot(SnapshotPayload),
    Delta(DeltaPayload),
}

impl LiveEnvelope {
    pub fn from_event(event: PositionEvent) -> Self {
        match event {
            PositionEvent::Snapshot {
                positions,
                emitted_at,
            } => LiveEnvelope::Snapshot(SnapshotPayload {
                positions: positions
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.into()))
                    .collect(),
                emitted_at,
            }),
            PositionEvent::Delta {
                changed,
                removed,
                emitted_at,
            } => LiveEnvelope::Delta(DeltaPayload {
                changed: changed
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.into()))
                    .collect(),
                removed: removed.into_iter().map(|i| i.to_string()).collect(),
                emitted_at,
            }),
        }
    }

    pub fn event_name(&self) -> &'static str {
        match self {
            LiveEnvelope::Snapshot(_) => "snapshot",
            LiveEnvelope::Delta(_) => "delta",
        }
    }

    pub fn payload_json(&self) -> Result<String, serde_json::Error> {
        match self {
            LiveEnvelope::Snapshot(s) => serde_json::to_string(s),
            LiveEnvelope::Delta(d) => serde_json::to_string(d),
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    use flightradar_domain::Icao24;

    fn t() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn lp(lat: f64) -> LivePosition {
        LivePosition {
            lat,
            lon: 0.0,
            alt_ft: None,
            ground_speed_kt: None,
            track_deg: None,
            callsign: None,
            category: None,
            updated_at: t(),
        }
    }

    #[test]
    fn snapshot_envelope_carries_positions_keyed_by_icao() {
        let mut positions = HashMap::new();
        positions.insert(Icao24::new("ABCDEF").unwrap(), lp(1.0));
        let env = LiveEnvelope::from_event(PositionEvent::Snapshot {
            positions,
            emitted_at: t(),
        });
        assert_eq!(env.event_name(), "snapshot");
        let json = env.payload_json().unwrap();
        assert!(json.contains(r#""ABCDEF""#));
    }

    #[test]
    fn delta_envelope_carries_changed_and_removed() {
        let mut changed = HashMap::new();
        changed.insert(Icao24::new("ABCDEF").unwrap(), lp(1.0));
        let env = LiveEnvelope::from_event(PositionEvent::Delta {
            changed,
            removed: vec![Icao24::new("123456").unwrap()],
            emitted_at: t(),
        });
        assert_eq!(env.event_name(), "delta");
        let json = env.payload_json().unwrap();
        assert!(json.contains(r#""ABCDEF""#));
        assert!(json.contains(r#""123456""#));
    }

    #[test]
    fn live_position_dto_maps_all_fields() {
        let dto: LivePositionDto = lp(47.0).into();
        assert_eq!(dto.lat, 47.0);
        assert!(dto.callsign.is_none());
    }
}
