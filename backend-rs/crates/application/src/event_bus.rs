//! In-process `PositionEventBus` implementation backed by
//! `tokio::sync::broadcast` for fan-out and `LiveState` for the initial
//! snapshot sent to fresh subscribers.
//!
//! Properties:
//! - **Lossy by design**: slow subscribers receive `Err(Lagged)` instead of
//!   blocking the publisher (the SSE handler can recover by re-reading the
//!   snapshot and re-subscribing).
//! - **Snapshot-on-subscribe**: every new subscriber gets a `Snapshot`
//!   event derived from the current `LiveState` before live deltas start
//!   flowing — so a connecting SSE client sees the whole picture
//!   immediately, no race against the next tick.
//! - **No back-pressure on the updater**: publishing is `O(1)`.

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use flightradar_domain::ports::event_bus::{
    EventBusError, EventStream, PositionEvent, PositionEventBus,
};

use crate::live_state::LiveState;

const DEFAULT_CAPACITY: usize = 1024;

/// Concrete `PositionEventBus`. Cheap to clone — all clones share the same
/// underlying broadcast channel.
#[derive(Clone)]
pub struct TokioBroadcastBus {
    sender: broadcast::Sender<PositionEvent>,
    live: LiveState,
}

impl std::fmt::Debug for TokioBroadcastBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokioBroadcastBus")
            .field("receiver_count", &self.sender.receiver_count())
            .finish_non_exhaustive()
    }
}

impl TokioBroadcastBus {
    pub fn new(live: LiveState) -> Self {
        Self::with_capacity(live, DEFAULT_CAPACITY)
    }

    pub fn with_capacity(live: LiveState, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, live }
    }

    /// Current number of live subscribers — useful for metrics.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Build a snapshot event from the current live state. Public so the
    /// SSE handler can synthesise its own initial event after a `Lagged`
    /// recovery.
    pub fn current_snapshot(&self) -> PositionEvent {
        let snap = self.live.read();
        let emitted_at = snap
            .generated_at()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        PositionEvent::Snapshot {
            positions: snap.positions().clone(),
            emitted_at,
        }
    }
}

impl PositionEventBus for TokioBroadcastBus {
    fn publish(&self, event: PositionEvent) {
        // Ignore "no subscribers" — that's normal during startup or when
        // no SSE clients are connected. Other errors cannot happen on send
        // (broadcast::send drops events for slow receivers internally).
        let _ = self.sender.send(event);
    }

    fn subscribe(&self) -> EventStream {
        let rx = self.sender.subscribe();
        let initial = self.current_snapshot();

        // Lift the broadcast receiver to a Stream, map driver errors to
        // domain errors, and prepend the initial snapshot.
        let live = BroadcastStream::new(rx).map(|item| match item {
            Ok(event) => Ok(event),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
                Err(EventBusError::Lagged)
            }
        });

        let stream = tokio_stream::iter([Ok(initial)]).chain(live);
        let pinned: Pin<Box<dyn Stream<Item = Result<PositionEvent, EventBusError>> + Send>> =
            Box::pin(stream);
        pinned
    }
}

/// Convenience: cheap shared handle.
pub type SharedEventBus = Arc<TokioBroadcastBus>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)] // positions are bit-identical
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use time::OffsetDateTime;
    use tokio_stream::StreamExt;

    use flightradar_domain::{Icao24, LivePosition, LiveSnapshot};

    use super::*;

    fn t() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    }

    fn snap_with(icao: &str, lat: f64) -> LiveSnapshot {
        let mut map = HashMap::new();
        map.insert(
            Icao24::new(icao).unwrap(),
            LivePosition {
                lat,
                lon: 0.0,
                alt_ft: None,
                ground_speed_kt: None,
                track_deg: None,
                callsign: None,
                category: None,
                updated_at: t(),
            },
        );
        LiveSnapshot::new(map, t())
    }

    fn delta(icao: &str, lat: f64) -> PositionEvent {
        let mut changed = HashMap::new();
        changed.insert(
            Icao24::new(icao).unwrap(),
            LivePosition {
                lat,
                lon: 0.0,
                alt_ft: None,
                ground_speed_kt: None,
                track_deg: None,
                callsign: None,
                category: None,
                updated_at: t(),
            },
        );
        PositionEvent::Delta {
            changed,
            removed: vec![],
            emitted_at: t(),
        }
    }

    #[tokio::test]
    async fn fresh_subscriber_receives_initial_snapshot() {
        let live = LiveState::empty();
        live.publish(snap_with("ABCDEF", 1.0));
        let bus = TokioBroadcastBus::new(live);

        let mut stream = bus.subscribe();
        let first = stream.next().await.unwrap().unwrap();
        match first {
            PositionEvent::Snapshot { positions, .. } => {
                assert_eq!(positions.len(), 1);
                assert!(positions.contains_key(&Icao24::new("ABCDEF").unwrap()));
            }
            other @ PositionEvent::Delta { .. } => panic!("expected Snapshot, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn subscriber_sees_subsequent_publishes() {
        let bus = TokioBroadcastBus::new(LiveState::empty());
        let mut stream = bus.subscribe();
        let _initial = stream.next().await.unwrap().unwrap();

        bus.publish(delta("ABCDEF", 2.0));

        let next = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        match next {
            PositionEvent::Delta { changed, .. } => {
                let entry = changed.get(&Icao24::new("ABCDEF").unwrap()).unwrap();
                assert_eq!(entry.lat, 2.0);
            }
            other @ PositionEvent::Snapshot { .. } => panic!("expected Delta, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn multiple_subscribers_each_receive_published_events() {
        let bus = TokioBroadcastBus::new(LiveState::empty());
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        // Drain the snapshots.
        let _ = a.next().await.unwrap();
        let _ = b.next().await.unwrap();

        bus.publish(delta("ABCDEF", 3.0));

        let ea = tokio::time::timeout(Duration::from_secs(1), a.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let eb = tokio::time::timeout(Duration::from_secs(1), b.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(ea, PositionEvent::Delta { .. }));
        assert!(matches!(eb, PositionEvent::Delta { .. }));
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_does_not_panic() {
        let bus = TokioBroadcastBus::new(LiveState::empty());
        bus.publish(delta("ABCDEF", 1.0)); // no receivers — must not panic
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn slow_subscriber_receives_lagged() {
        // Small capacity so we can overflow quickly.
        let bus = TokioBroadcastBus::with_capacity(LiveState::empty(), 2);
        let mut stream = bus.subscribe();
        let _initial = stream.next().await.unwrap().unwrap();

        // Publish more than capacity without consuming.
        for i in 0..10 {
            bus.publish(delta("ABCDEF", f64::from(i)));
        }

        // Read until we observe a Lagged error.
        let mut saw_lagged = false;
        for _ in 0..12 {
            match tokio::time::timeout(Duration::from_millis(200), stream.next()).await {
                Ok(Some(Err(EventBusError::Lagged))) => {
                    saw_lagged = true;
                    break;
                }
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(EventBusError::Closed)) | None) | Err(_) => break,
            }
        }
        assert!(saw_lagged, "expected at least one Lagged on overflow");
    }

    #[tokio::test]
    async fn current_snapshot_reflects_latest_live_state() {
        let live = LiveState::empty();
        let bus = TokioBroadcastBus::new(live.clone());
        live.publish(snap_with("ABCDEF", 5.0));

        match bus.current_snapshot() {
            PositionEvent::Snapshot { positions, .. } => {
                assert_eq!(
                    positions.get(&Icao24::new("ABCDEF").unwrap()).unwrap().lat,
                    5.0
                );
            }
            other @ PositionEvent::Delta { .. } => panic!("expected Snapshot, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn subscriber_count_tracks_live_subscribers() {
        let bus = TokioBroadcastBus::new(LiveState::empty());
        assert_eq!(bus.subscriber_count(), 0);
        let s1 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        let s2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
        drop(s1);
        drop(s2);
        // BroadcastStream wraps the receiver; once dropped, the count goes
        // down to zero on the next publish.
        bus.publish(delta("ABCDEF", 1.0));
        assert_eq!(bus.subscriber_count(), 0);
    }
}
