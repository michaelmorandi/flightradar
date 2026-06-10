//! Admin endpoints (require role=admin).

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Deserializer, Serialize};

use flightradar_application::{AdminStats, AircraftPatch};
use flightradar_domain::Icao24;

/// Distinguish "field missing" (`None`) from "field set to null"
/// (`Some(None)`) in `#[derive(Deserialize)]`. Without this, serde
/// collapses both to `None` and the admin editor has no way to clear
/// a field.
#[allow(clippy::option_option)] // tri-state on purpose
fn double_option<'de, T, D>(de: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

use crate::dto::aircraft::AircraftDto;
use crate::error::ApiError;
use crate::extractors::AdminUser;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AdminStatsDto {
    pub flight_count: u64,
}

impl From<AdminStats> for AdminStatsDto {
    fn from(s: AdminStats) -> Self {
        Self {
            flight_count: s.flight_count,
        }
    }
}

pub async fn stats(
    State(state): State<AppState>,
    _: AdminUser,
) -> Result<Json<AdminStatsDto>, ApiError> {
    let s = state.admin.stats().await?;
    Ok(Json(s.into()))
}

pub async fn get_aircraft(
    State(state): State<AppState>,
    _: AdminUser,
    Path(icao): Path<String>,
) -> Result<Json<AircraftDto>, ApiError> {
    let icao24 = Icao24::new(&icao).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let ac = state.aircraft.get(&icao24).await?;
    Ok(Json(ac.into()))
}

#[derive(Debug, Deserialize, Default)]
#[allow(clippy::option_option)] // tri-state on purpose; see admin.rs
pub struct AircraftPatchRequest {
    #[serde(default, deserialize_with = "double_option")]
    pub registration: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub type_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub type_description: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub operator: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub designator: Option<Option<String>>,
}

impl From<AircraftPatchRequest> for AircraftPatch {
    fn from(req: AircraftPatchRequest) -> Self {
        Self {
            registration: req.registration,
            type_code: req.type_code,
            type_description: req.type_description,
            operator: req.operator,
            designator: req.designator,
        }
    }
}

pub async fn put_aircraft(
    State(state): State<AppState>,
    _: AdminUser,
    Path(icao): Path<String>,
    Json(req): Json<AircraftPatchRequest>,
) -> Result<Json<AircraftDto>, ApiError> {
    let icao24 = Icao24::new(&icao).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let updated = state.admin.update_aircraft(&icao24, req.into()).await?;
    Ok(Json(updated.into()))
}
