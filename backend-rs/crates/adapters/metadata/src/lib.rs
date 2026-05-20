//! Aircraft metadata adapters.
//!
//! All metadata flows through `nighthawk-proxy`, which fans out to OpenSky,
//! hexdb, planespotters, etc. This adapter exposes one [`NighthawkSource`]
//! per sub-source endpoint (discovered at startup) and a static
//! [`NighthawkClient`] for talking to the proxy itself.

pub mod nighthawk;

pub use nighthawk::{
    discover_nighthawk_sources, parse_aircraft_payload, NighthawkClient, NighthawkSource,
};
