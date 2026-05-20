use async_trait::async_trait;
use thiserror::Error;

use crate::entities::aircraft::Aircraft;
use crate::value_objects::Icao24;

#[derive(Debug, Error)]
pub enum MetadataError {
    #[error("metadata source unavailable: {0}")]
    Unavailable(String),

    #[error("rate limited by upstream")]
    RateLimited,

    #[error("malformed payload: {0}")]
    MalformedPayload(String),

    #[error("metadata source transport error: {0}")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Provider of aircraft master data. Today only one impl exists
/// (`nighthawk-proxy`), but the trait keeps the door open for additions
/// without touching the crawler.
#[async_trait]
pub trait MetadataSource: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;

    /// Fetch what is known about `icao24`. `Ok(None)` means "not found" —
    /// distinct from `Err`, which means the source itself failed.
    async fn fetch(&self, icao24: &Icao24) -> Result<Option<Aircraft>, MetadataError>;
}
