//! MongoDB adapter.
//!
//! Implements every repository port from `flightradar-domain` against the
//! official `mongodb` driver. The BSON ↔ domain mapping lives in [`codec`]
//! as pure functions so it can be unit-tested without a live database;
//! the [`repositories`] module wraps `Collection`s and only adds the I/O
//! envelope.

pub mod codec;
pub mod collections;
pub mod connection;
pub mod error;
pub mod repositories;
pub mod schema;

pub use connection::{MongoConfig, MongoConnection, MongoConnectionError};
pub use repositories::aircraft::MongoAircraftRepository;
pub use repositories::crawler_log::MongoCrawlerLogRepository;
pub use repositories::crawler_queue::MongoCrawlerQueueRepository;
pub use repositories::flight::MongoFlightRepository;
pub use repositories::position::MongoPositionRepository;
pub use repositories::user::MongoUserRepository;
pub use schema::ensure_schema;
