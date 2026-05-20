//! Read-side use cases.
//!
//! Thin wrappers around repositories and the live state. Kept separate from
//! the write-side use cases (FlightUpdater, AircraftCrawler, AuthService)
//! to make CQRS-style boundaries explicit and reads cheap to test.

pub mod aircraft_query;
pub mod airline_query;
pub mod flight_query;
