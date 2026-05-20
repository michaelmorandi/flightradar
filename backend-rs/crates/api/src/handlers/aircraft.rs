//! Aircraft endpoints: single + bulk lookup.

use axum::extract::{Path, State};
use axum::Json;

use flightradar_domain::Icao24;

use crate::dto::aircraft::{AircraftDto, BulkAircraftRequest, BulkAircraftResponse};
use crate::error::ApiError;
use crate::extractors::Authenticated;
use crate::state::AppState;

pub async fn get_one(
    State(state): State<AppState>,
    _: Authenticated,
    Path(icao): Path<String>,
) -> Result<Json<AircraftDto>, ApiError> {
    let icao24 = Icao24::new(&icao).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let ac = state.aircraft.get(&icao24).await?;
    Ok(Json(ac.into()))
}

pub async fn get_many(
    State(state): State<AppState>,
    _: Authenticated,
    Json(req): Json<BulkAircraftRequest>,
) -> Result<Json<BulkAircraftResponse>, ApiError> {
    let icaos: Result<Vec<_>, _> = req.icao24s.iter().map(|s| Icao24::new(s)).collect();
    let icaos = icaos.map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let requested = icaos.len();
    let aircraft = state.aircraft.get_many(&icaos).await?;
    let found = aircraft.len();
    Ok(Json(BulkAircraftResponse {
        aircraft: aircraft.into_iter().map(Into::into).collect(),
        requested,
        found,
    }))
}
