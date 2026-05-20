//! Application layer.
//!
//! Use cases that orchestrate domain entities and ports. No knowledge of
//! HTTP, MongoDB, gRPC, etc. — those are injected as `Arc<dyn Trait>`.
//!
//! Use cases will land here in subsequent commits:
//! - `flight_updater` — radar stream → state → event bus → persistence
//! - `aircraft_crawler` — queue → metadata source → repository
//! - `auth` — anonymous / admin login
//! - read-side: `flight_query`, `aircraft_query`, `airline_query`
