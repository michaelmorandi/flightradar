//! Lock-free live state store.
//!
//! Single writer (the `FlightUpdater`) publishes immutable snapshots; many
//! readers (HTTP handlers, SSE handlers) consume them with sub-microsecond
//! latency via `arc_swap::ArcSwap`.

use std::sync::Arc;

use arc_swap::ArcSwap;

use flightradar_domain::LiveSnapshot;

/// Cheaply cloneable handle to the current live snapshot. All clones share
/// the same underlying `ArcSwap`.
#[derive(Debug, Clone)]
pub struct LiveState {
    inner: Arc<ArcSwap<LiveSnapshot>>,
}

impl LiveState {
    pub fn new(initial: LiveSnapshot) -> Self {
        Self {
            inner: Arc::new(ArcSwap::from_pointee(initial)),
        }
    }

    pub fn empty() -> Self {
        Self::new(LiveSnapshot::empty())
    }

    /// Return the current snapshot. The returned `Arc` is detached from the
    /// store — subsequent `publish` calls do not invalidate it.
    pub fn read(&self) -> Arc<LiveSnapshot> {
        self.inner.load_full()
    }

    /// Atomically replace the current snapshot. Old readers retain their
    /// view; new readers see the updated one.
    pub fn publish(&self, next: LiveSnapshot) {
        self.inner.store(Arc::new(next));
    }
}

impl Default for LiveState {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // intentional: positions are bit-identical
mod tests {
    use std::collections::HashMap;

    use flightradar_domain::{Icao24, LivePosition};
    use time::OffsetDateTime;

    use super::*;

    fn now() -> OffsetDateTime {
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
                updated_at: now(),
            },
        );
        LiveSnapshot::new(map, now())
    }

    #[test]
    fn empty_state_has_no_positions() {
        let state = LiveState::empty();
        assert!(state.read().is_empty());
    }

    #[test]
    fn publish_replaces_snapshot() {
        let state = LiveState::empty();
        state.publish(snap_with("ABCDEF", 1.0));
        assert_eq!(state.read().len(), 1);

        state.publish(snap_with("ABCDEF", 2.0));
        assert_eq!(
            state
                .read()
                .get(&Icao24::new("ABCDEF").unwrap())
                .unwrap()
                .lat,
            2.0
        );
    }

    #[test]
    fn clones_share_underlying_store() {
        let a = LiveState::empty();
        let b = a.clone();
        a.publish(snap_with("ABCDEF", 1.0));
        assert_eq!(b.read().len(), 1);
    }

    #[test]
    fn detached_read_survives_publish() {
        let state = LiveState::empty();
        state.publish(snap_with("ABCDEF", 1.0));
        let detached = state.read();
        state.publish(snap_with("ABCDEF", 99.0));
        // Original reader still holds its view.
        assert_eq!(
            detached.get(&Icao24::new("ABCDEF").unwrap()).unwrap().lat,
            1.0
        );
    }
}
