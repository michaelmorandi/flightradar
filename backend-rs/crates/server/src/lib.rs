//! Server composition root, exposed as a library so integration tests can
//! drive the same wiring that `main` builds.

pub mod composition;
pub mod config;
pub mod observability;
pub mod supervisor;

pub use composition::{build_app, ComposedApp};
pub use config::{Config, RadarKind};
