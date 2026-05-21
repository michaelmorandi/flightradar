//! Admin endpoints (require role=admin).

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use flightradar_application::{AdminStats, AircraftPatch};
use flightradar_domain::Icao24;

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
pub struct AircraftPatchRequest {
    pub registration: Option<String>,
    pub type_code: Option<String>,
    pub type_description: Option<String>,
    pub operator: Option<String>,
    pub designator: Option<String>,
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
