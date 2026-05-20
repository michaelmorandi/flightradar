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

pub type PositionStream =
    Pin<Box<dyn Stream<Item = Result<PositionReport, RadarError>> + Send + 'static>>;

/// A live ADS-B position source. Implementations decide whether they poll
/// (dump1090) or maintain a persistent stream (gRPC).
#[async_trait]
pub trait RadarSource: Send + Sync + std::fmt::Debug {
    /// Human-readable identifier, used in logs/metrics (`"dump1090"`, `"grpc"`).
    fn name(&self) -> &'static str;

    /// Begin streaming. May be called once per process; implementations
    /// should reconnect internally and surface only fatal errors.
    async fn stream(&self) -> Result<PositionStream, RadarError>;
}
