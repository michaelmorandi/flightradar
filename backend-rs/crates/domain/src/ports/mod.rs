//! Ports — the traits that the application layer depends on.
//!
//! Concrete implementations live in `adapters/*`. This is the only place
//! where the domain crate names "outside" capabilities.

pub mod airline_directory;
pub mod auth;
pub mod clock;
pub mod event_bus;
pub mod metadata_source;
pub mod radar_source;
pub mod repositories;
