//! Single source of truth for collection names. Centralised so changing a
//! name only touches one file.

pub const FLIGHTS: &str = "flights";
pub const POSITIONS: &str = "positions";
pub const AIRCRAFT: &str = "aircraft";
pub const CRAWLER_QUEUE: &str = "aircraft_to_process";
pub const CRAWLER_LOGS: &str = "crawler_logs";
pub const USERS: &str = "users";
