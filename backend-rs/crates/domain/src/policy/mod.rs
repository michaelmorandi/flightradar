//! Pure domain policies.
//!
//! These are stateful but I/O-free: they take the current time as input,
//! never call it, so tests are deterministic.

pub mod callsign;
pub mod circuit_breaker;
pub mod modes;
