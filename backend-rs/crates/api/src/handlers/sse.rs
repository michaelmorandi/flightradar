//! Server-Sent Events: live positions stream.
//!
//! Two endpoints share the same underlying broadcast subscription:
//! - `GET /live/stream` — all positions.
//! - `GET /live/stream/{icao24}` — single-aircraft filter, applied
//!   server-side so the client only sees one row.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::response::Response;
use futures::stream::Stream;
use futures::StreamExt;
use tracing::warn;

use flightradar_domain::ports::event_bus::{PositionEvent, PositionEventBus};
use flightradar_domain::Icao24;

use crate::dto::live::LiveEnvelope;
use crate::error::ApiError;
use crate::extractors::Authenticated;
use crate::state::AppState;

pub async fn stream_all(
    State(state): State<AppState>,
    _: Authenticated,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    Ok(make_sse(&state, None))
}

pub async fn stream_one(
    State(state): State<AppState>,
    _: Authenticated,
    Path(icao): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let icao24 = Icao24::new(&icao).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(make_sse(&state, Some(icao24)))
}

fn make_sse(
    state: &AppState,
    filter: Option<Icao24>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let upstream = state.events.subscribe();
    let stream = upstream.filter_map(move |item| {
        let filter = filter.clone();
        async move {
            match item {
                Ok(event) => render(event, filter.as_ref()).map(Ok),
                Err(err) => {
                    warn!(error = %err, "live stream subscriber error");
                    None
                }
            }
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// Map a `PositionEvent` to an `Event` (named + JSON body). Returns `None`
/// when a filter is set and the event contains nothing for that ICAO24 —
/// callers should not see empty deltas just because the rest of the world
/// moved.
fn render(event: PositionEvent, filter: Option<&Icao24>) -> Option<Event> {
    let filtered = if let Some(icao) = filter {
        filter_event(event, icao)?
    } else {
        event
    };
    let envelope = LiveEnvelope::from_event(filtered);
    let payload = envelope.payload_json().ok()?;
    Some(Event::default().event(envelope.event_name()).data(payload))
}

fn filter_event(event: PositionEvent, icao: &Icao24) -> Option<PositionEvent> {
    match event {
        PositionEvent::Snapshot {
            mut positions,
            emitted_at,
        } => {
            positions.retain(|k, _| k == icao);
            // Always send the (possibly empty) snapshot — the client needs
            // to know whether the aircraft is currently being tracked.
            Some(PositionEvent::Snapshot {
                positions,
                emitted_at,
            })
        }
        PositionEvent::Delta {
            mut changed,
            mut removed,
            emitted_at,
        } => {
            changed.retain(|k, _| k == icao);
            removed.retain(|k| k == icao);
            if changed.is_empty() && removed.is_empty() {
                None
            } else {
                Some(PositionEvent::Delta {
                    changed,
                    removed,
                    emitted_at,
                })
            }
        }
    }
}

/// `IntoResponse` impl is provided by `Sse`. This trivial type alias keeps
/// the `Result<…, ApiError>` ergonomic in router declarations.
pub type SseResponse = Response;

impl From<ApiError> for SseResponse {
    fn from(err: ApiError) -> Self {
        err.into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use flightradar_domain::LivePosition;
    use time::OffsetDateTime;

    use super::*;

    fn t() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn lp() -> LivePosition {
        LivePosition {
            lat: 47.0,
            lon: 8.0,
            alt_ft: None,
            ground_speed_kt: None,
            track_deg: None,
            callsign: None,
            category: None,
            updated_at: t(),
        }
    }

    #[test]
    fn filter_keeps_only_target_icao_in_delta() {
        let mut changed = HashMap::new();
        changed.insert(Icao24::new("ABCDEF").unwrap(), lp());
        changed.insert(Icao24::new("123456").unwrap(), lp());
        let event = PositionEvent::Delta {
            changed,
            removed: vec![Icao24::new("AAAAAA").unwrap()],
            emitted_at: t(),
        };
        let filtered =
            filter_event(event, &Icao24::new("ABCDEF").unwrap()).expect("non-empty delta");
        match filtered {
            PositionEvent::Delta {
                changed, removed, ..
            } => {
                assert_eq!(changed.len(), 1);
                assert!(changed.contains_key(&Icao24::new("ABCDEF").unwrap()));
                assert!(removed.is_empty());
            }
            PositionEvent::Snapshot { .. } => panic!("expected Delta"),
        }
    }

    #[test]
    fn filter_drops_delta_with_no_matching_icao() {
        let mut changed = HashMap::new();
        changed.insert(Icao24::new("123456").unwrap(), lp());
        let event = PositionEvent::Delta {
            changed,
            removed: vec![],
            emitted_at: t(),
        };
        let filtered = filter_event(event, &Icao24::new("ABCDEF").unwrap());
        assert!(filtered.is_none());
    }

    #[test]
    fn filter_keeps_empty_snapshot_for_target_icao() {
        let mut positions = HashMap::new();
        positions.insert(Icao24::new("123456").unwrap(), lp());
        let event = PositionEvent::Snapshot {
            positions,
            emitted_at: t(),
        };
        // Always returned (so the client knows it isn't tracking this one).
        let filtered =
            filter_event(event, &Icao24::new("ABCDEF").unwrap()).expect("snapshot always sent");
        match filtered {
            PositionEvent::Snapshot { positions, .. } => assert!(positions.is_empty()),
            PositionEvent::Delta { .. } => panic!("expected Snapshot"),
        }
    }

    #[test]
    fn render_produces_named_event_with_json_body() {
        let mut changed = HashMap::new();
        changed.insert(Icao24::new("ABCDEF").unwrap(), lp());
        let event = PositionEvent::Delta {
            changed,
            removed: vec![],
            emitted_at: t(),
        };
        // Just verify render() builds *some* Event without panicking.
        let _ = render(event, None).expect("renders an event");
    }
}
