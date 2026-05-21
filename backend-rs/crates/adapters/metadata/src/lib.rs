//! Aircraft metadata adapters.
//!
//! All metadata flows through `nighthawk-proxy`, which fans out to OpenSky,
//! hexdb, planespotters, etc. This adapter exposes one [`NighthawkSource`]
//! per sub-source endpoint (discovered at startup) and a static
//! [`NighthawkClient`] for talking to the proxy itself.
//!
//! [`StaticAirlineDirectory`] loads the operator/airline reference data
//! from the legacy `operators.json` shape and serves it from memory.

pub mod nighthawk;
pub mod static_airlines;

pub use nighthawk::{
    discover_nighthawk_sources, parse_aircraft_payload, NighthawkClient, NighthawkSource,
};
pub use static_airlines::StaticAirlineDirectory;
