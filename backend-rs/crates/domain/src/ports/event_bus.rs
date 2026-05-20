//! Event bus port — the seam between the `FlightUpdater` and the SSE layer.
//!
//! Implementations are lossy by design: a slow subscriber must never block
//! ingestion. The concrete impl wraps `tokio::sync::broadcast`.

use std::collections::HashMap;
use std::pin::Pin;

use futures_core::Stream;
use thiserror::Error;
use time::OffsetDateTime;

use crate::entities::live_snapshot::LivePosition;
use crate::value_objects::Icao24;

/// Payload emitted by the `FlightUpdater` on every tick.
#[derive(Debug, Clone)]
pub enum PositionEvent {
    /// Initial snapshot sent to a freshly subscribed client.
    Snapshot {
        positions: HashMap<Icao24, LivePosition>,
        emitted_at: OffsetDateTime,
    },
    /// Delta with only the positions that changed this tick, plus the ICAOs
    /// that disappeared from the live picture.
    Delta {
        changed: HashMap<Icao24, LivePosition>,
        removed: Vec<Icao24>,
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
