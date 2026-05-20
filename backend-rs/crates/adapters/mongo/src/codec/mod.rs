//! BSON ↔ domain conversion. One module per stored entity. Pure
//! functions: no I/O, no driver types beyond [`bson::Document`].

pub mod aircraft;
pub mod crawler;
pub mod flight;
pub mod position;
pub mod user;
