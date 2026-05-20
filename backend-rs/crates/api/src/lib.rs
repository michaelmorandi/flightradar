//! HTTP / SSE API.
//!
//! Thin translation layer: Axum router + DTOs + mappers + middleware. All
//! business logic lives in `flightradar-application`; handlers do nothing
//! beyond extract → call use case → map response.

pub mod dto;
pub mod error;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod router;
pub mod state;

pub use error::ApiError;
pub use router::build_router;
pub use state::{AppState, AuthState, BuildInfo};
