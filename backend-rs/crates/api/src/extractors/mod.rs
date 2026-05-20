//! Custom Axum extractors: auth identity + pagination.

pub mod auth;
pub mod pagination;

pub use auth::{AdminUser, Authenticated};
pub use pagination::Pagination;
