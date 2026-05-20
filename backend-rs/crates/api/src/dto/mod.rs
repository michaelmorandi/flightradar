//! Request / response shapes.
//!
//! DTOs are kept separate from domain entities. Mappers (`From` impls /
//! free functions) live here so domain types never leak storage or wire
//! concerns and so the JSON shape can change without touching `domain`.

pub mod aircraft;
pub mod airline;
pub mod auth;
pub mod common;
pub mod flight;
pub mod live;
