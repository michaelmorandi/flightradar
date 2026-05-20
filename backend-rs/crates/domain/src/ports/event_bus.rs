//! Event bus port — the seam between the `FlightUpdater` and the SSE layer.
//!
//! Implementations are lossy by design: a slow subscriber must never block
//! ingestion. The concrete impl wraps `tokio::sync::broadcast`.

use std::collections::HashMap;
use std::pin::Pin;

use futures_core::Stream;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

use crate::entities::flight::FlightId;
use crate::entities::position_report::AircraftCategory;
use crate::value_objects::{Callsign, Icao24};

/// One element of a `PositionDiff` map. Compact on purpose — broadcasts
/// happen every tick.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LivePosition {
    pub icao24: Icao24,
    pub lat: f64,
    pub lon: f64,
    pub alt_ft: Option<i32>,
    pub ground_speed_kt: Option<f64>,
    pub track_deg: Option<f64>,
    pub callsign: Option<Callsign>,
    pub category: Option<AircraftCategory>,
}

/// Payload emitted by the `FlightUpdater` on every tick.
#[derive(Debug, Clone)]
pub enum PositionEvent {
    /// Initial snapshot sent to a freshly subscribed client.
    Snapshot {
        positions: HashMap<FlightId, LivePosition>,
        emitted_at: OffsetDateTime,
    },
    /// Delta with only the positions that changed this tick.
    Delta {
        positions: HashMap<FlightId, LivePosition>,
        emitted_at: OffsetDateTime,
    },
}

#[derive(Debug, Error)]
pub enum EventBusError {
    #[error("subscriber lagged and dropped messages")]
    Lagged,

    #[error("event bus closed")]
    Closed,
}

pub type EventStream =
    Pin<Box<dyn Stream<Item = Result<PositionEvent, EventBusError>> + Send + 'static>>;

/// Publish/subscribe seam for live position events.
pub trait PositionEventBus: Send + Sync + std::fmt::Debug {
    fn publish(&self, event: PositionEvent);

    /// Subscribe to subsequent events. Implementations should also send the
    /// caller a `Snapshot` of the current state as the first item.
    fn subscribe(&self) -> EventStream;
}
