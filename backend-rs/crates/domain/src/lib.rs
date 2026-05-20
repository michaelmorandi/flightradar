//! Domain layer.
//!
//! Pure types, value objects, entities, policies, and the ports (traits)
//! that the application layer uses to talk to the outside world. No I/O,
//! no runtime, no framework dependencies.

pub mod entities;
pub mod error;
pub mod policy;
pub mod ports;
pub mod value_objects;

pub use entities::aircraft::{Aircraft, AircraftSource};
pub use entities::airline::Airline;
pub use entities::flight::{Flight, FlightId};
pub use entities::live_snapshot::{LivePosition, LiveSnapshot};
pub use entities::position_report::{AircraftCategory, PositionReport};
pub use entities::user::{Role, User, UserId};
pub use error::DomainError;
pub use value_objects::{AirlineIcao, Callsign, Icao24};
