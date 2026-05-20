//! Airline endpoints: get + search.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use flightradar_domain::AirlineIcao;

use crate::dto::airline::AirlineDto;
use crate::error::ApiError;
use crate::extractors::Authenticated;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<u32>,
}

pub async fn get_one(
    State(state): State<AppState>,
    _: Authenticated,
    Path(icao): Path<String>,
) -> Result<Json<AirlineDto>, ApiError> {
    let icao = AirlineIcao::new(&icao).map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let airline = state.airlines.get(&icao).await?;
    Ok(Json(airline.into()))
}

pub async fn search(
    State(state): State<AppState>,
    _: Authenticated,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<AirlineDto>>, ApiError> {
    let results = state.airlines.search(&q.q, q.limit).await?;
    Ok(Json(results.into_iter().map(Into::into).collect()))
}
