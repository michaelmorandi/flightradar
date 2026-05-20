//! Repository ports — narrow, role-based interfaces. Concrete Mongo impls
//! live in `adapters/mongo`. Each trait is small on purpose (ISP).

use async_trait::async_trait;
use thiserror::Error;
use time::OffsetDateTime;

use crate::entities::aircraft::Aircraft;
use crate::entities::flight::{Flight, FlightId};
use crate::entities::position_report::PositionReport;
use crate::entities::user::{User, UserId};
use crate::value_objects::{AirlineIcao, Icao24};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("entity not found")]
    NotFound,

    #[error("constraint violation: {0}")]
    Conflict(String),

    #[error("database unavailable")]
    Unavailable,

    #[error("database error: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

pub type RepoResult<T> = Result<T, RepositoryError>;

#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Default)]
pub struct FlightFilter {
    pub icao24: Option<Icao24>,
    pub airline: Option<AirlineIcao>,
    pub military_only: bool,
    pub exclude_live_since: Option<OffsetDateTime>,
    pub free_text: Option<String>,
}

// ---------------------------------------------------------------------------
// Flights
// ---------------------------------------------------------------------------

#[async_trait]
pub trait FlightRepository: Send + Sync + std::fmt::Debug {
    async fn upsert(&self, flight: &Flight) -> RepoResult<()>;
    async fn find_by_id(&self, id: &FlightId) -> RepoResult<Flight>;
    async fn find_open_for_icao24(&self, icao24: &Icao24) -> RepoResult<Option<Flight>>;
    async fn list(&self, filter: &FlightFilter, page: PageRequest) -> RepoResult<Page<Flight>>;
}

// ---------------------------------------------------------------------------
// Positions (time-series)
// ---------------------------------------------------------------------------

#[async_trait]
pub trait PositionRepository: Send + Sync + std::fmt::Debug {
    async fn append(&self, flight_id: &FlightId, position: &PositionReport) -> RepoResult<()>;
    async fn append_batch(&self, entries: &[(FlightId, PositionReport)]) -> RepoResult<()>;
    async fn history(&self, flight_id: &FlightId) -> RepoResult<Vec<PositionReport>>;
}

// ---------------------------------------------------------------------------
// Aircraft master data
// ---------------------------------------------------------------------------

#[async_trait]
pub trait AircraftRepository: Send + Sync + std::fmt::Debug {
    async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>>;
    async fn find_many(&self, icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>>;
    async fn upsert(&self, aircraft: &Aircraft) -> RepoResult<()>;
}

// ---------------------------------------------------------------------------
// Crawler queue & logs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CrawlerQueueEntry {
    pub icao24: Icao24,
    pub attempts: u32,
    pub last_attempt_at: Option<OffsetDateTime>,
}

#[async_trait]
pub trait CrawlerQueueRepository: Send + Sync + std::fmt::Debug {
    async fn enqueue(&self, icao24: &Icao24) -> RepoResult<()>;
    async fn next_batch(&self, batch_size: u32) -> RepoResult<Vec<CrawlerQueueEntry>>;
    async fn record_attempt(&self, icao24: &Icao24, success: bool) -> RepoResult<()>;
}

#[derive(Debug, Clone)]
pub struct CrawlerLogEntry {
    pub icao24: Icao24,
    pub source: String,
    pub success: bool,
    pub recorded_at: OffsetDateTime,
}

#[async_trait]
pub trait CrawlerLogRepository: Send + Sync + std::fmt::Debug {
    async fn record(&self, entry: &CrawlerLogEntry) -> RepoResult<()>;
    async fn recent_for(&self, icao24: &Icao24, limit: u32) -> RepoResult<Vec<CrawlerLogEntry>>;
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

#[async_trait]
pub trait UserRepository: Send + Sync + std::fmt::Debug {
    async fn find_by_id(&self, id: &UserId) -> RepoResult<Option<User>>;
    async fn find_by_email(&self, email: &str) -> RepoResult<Option<User>>;
    async fn upsert(&self, user: &User, hashed_password: Option<&str>) -> RepoResult<()>;
    async fn read_password_hash(&self, id: &UserId) -> RepoResult<Option<String>>;
    async fn touch_last_login(&self, id: &UserId, when: OffsetDateTime) -> RepoResult<()>;
}
