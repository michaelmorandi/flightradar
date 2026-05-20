use std::pin::Pin;

use async_trait::async_trait;
use futures_core::Stream;
use thiserror::Error;

use crate::entities::position_report::PositionReport;

#[derive(Debug, Error)]
pub enum RadarError {
    #[error("radar source is unavailable: {0}")]
    Unavailable(String),

    #[error("malformed payload from radar source: {0}")]
    MalformedPayload(String),

    #[error("radar source transport error: {0}")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Stream of position observations. Each item is one aircraft sighting —
/// streaming sources (gRPC) forward server-stream items directly; polling
/// sources (dump1090) emit each polled aircraft as a separate item.
///
/// Adapters are expected to reconnect internally on transient failures and
/// surface only fatal conditions by closing the stream. Consumers should
/// treat end-of-stream as a signal to back off and restart the source.
pub type PositionStream = Pin<Box<dyn Stream<Item = PositionReport> + Send + 'static>>;

/// A live ADS-B position source. Implementations choose how they get the
/// data — polled (dump1090) or persistent stream (gRPC) — and present a
/// uniform pull-based `Stream` to the application.
#[async_trait]
pub trait RadarSource: Send + Sync + std::fmt::Debug {
    /// Human-readable identifier, used in logs/metrics (`"dump1090"`, `"grpc"`).
    fn name(&self) -> &'static str;

    /// Begin a session. Called once per process; the returned stream is
    /// expected to live as long as the source is healthy.
    async fn stream(&self) -> Result<PositionStream, RadarError>;
}
