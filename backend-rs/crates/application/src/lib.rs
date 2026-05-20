//! Application layer.
//!
//! Use cases that orchestrate domain entities and ports. No knowledge of
//! HTTP, MongoDB, gRPC, or any other concrete I/O — those are injected as
//! `Arc<dyn Trait>`.

pub mod aircraft_crawler;
pub mod auth_service;
pub mod error;
pub mod event_bus;
pub mod flight_updater;
pub mod live_state;
pub mod queries;

pub use aircraft_crawler::{AircraftCrawler, AircraftCrawlerConfig};
pub use auth_service::AuthService;
pub use error::ApplicationError;
pub use event_bus::TokioBroadcastBus;
pub use flight_updater::{FlightUpdater, FlightUpdaterConfig, FlightUpdaterTickReport};
pub use live_state::LiveState;
pub use queries::aircraft_query::AircraftQuery;
pub use queries::airline_query::AirlineQuery;
pub use queries::flight_query::FlightQuery;
