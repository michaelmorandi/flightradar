//! Lookup of airline reference data. The current implementation will be a
//! one-shot JSON loader (`operators.json`), but the trait keeps the door
//! open for runtime-backed sources (DB, API).

use async_trait::async_trait;
use thiserror::Error;

use crate::entities::airline::Airline;
use crate::value_objects::AirlineIcao;

#[derive(Debug, Error)]
pub enum AirlineDirectoryError {
    #[error("airline directory unavailable: {0}")]
    Unavailable(String),
}

#[async_trait]
pub trait AirlineDirectory: Send + Sync + std::fmt::Debug {
    async fn find(&self, icao: &AirlineIcao) -> Result<Option<Airline>, AirlineDirectoryError>;

    /// Free-text search across airline name / ICAO / IATA. Implementations
    /// decide the matching strategy; the contract is "best-effort, capped".
    async fn search(&self, query: &str, limit: u32) -> Result<Vec<Airline>, AirlineDirectoryError>;

    async fn all(&self) -> Result<Vec<Airline>, AirlineDirectoryError>;
}
