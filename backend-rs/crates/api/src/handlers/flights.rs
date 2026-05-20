//! Flights endpoints: list + get + position history.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use flightradar_domain::ports::repositories::FlightFilter;
use flightradar_domain::{AirlineIcao, FlightId, Icao24};

use crate::dto::common::{PageInfo, PagedResponse};
use crate::dto::flight::{FlightDto, PositionDto};
use crate::error::ApiError;
use crate::extractors::{Authenticated, Pagination};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub icao24: Option<String>,
    pub airline: Option<String>,
    #[serde(default)]
    pub military_only: bool,
    pub q: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    Pagination(page): Pagination,
    _: Authenticated,
    Query(q): Query<ListQuery>,
) -> Result<Json<PagedResponse<FlightDto>>, ApiError> {
    let filter = build_filter(&q)?;
    let result = state.flights.list(&filter, page).await?;
    let info = PageInfo::from_page(&result);
    let items = result.items.into_iter().map(Into::into).collect();
    Ok(Json(PagedResponse { items, page: info }))
}

pub async fn get_one(
    State(state): State<AppState>,
    _: Authenticated,
    Path(id): Path<String>,
) -> Result<Json<FlightDto>, ApiError> {
    let flight = state.flights.get(&FlightId::new(id)).await?;
    Ok(Json(flight.into()))
}

pub async fn history(
    State(state): State<AppState>,
    _: Authenticated,
    Path(id): Path<String>,
) -> Result<Json<Vec<PositionDto>>, ApiError> {
    let positions = state.flights.history(&FlightId::new(id)).await?;
    Ok(Json(positions.into_iter().map(Into::into).collect()))
}

fn build_filter(q: &ListQuery) -> Result<FlightFilter, ApiError> {
    let icao24 = q
        .icao24
        .as_deref()
        .map(Icao24::new)
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    let airline = q
        .airline
        .as_deref()
        .map(AirlineIcao::new)
        .transpose()
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok(FlightFilter {
        icao24,
        airline,
        military_only: q.military_only,
        exclude_live_since: None,
        free_text: q.q.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_filter_parses_query_params() {
        let q = ListQuery {
            icao24: Some("ABCDEF".into()),
            airline: Some("AFR".into()),
            military_only: true,
            q: Some("search".into()),
        };
        let f = build_filter(&q).unwrap();
        assert_eq!(f.icao24.unwrap().as_str(), "ABCDEF");
        assert_eq!(f.airline.unwrap().as_str(), "AFR");
        assert!(f.military_only);
        assert_eq!(f.free_text.as_deref(), Some("search"));
    }

    #[test]
    fn build_filter_rejects_bad_icao24() {
        let q = ListQuery {
            icao24: Some("ZZZ".into()),
            airline: None,
            military_only: false,
            q: None,
        };
        assert!(matches!(
            build_filter(&q).unwrap_err(),
            ApiError::BadRequest(_)
        ));
    }

    #[test]
    fn build_filter_rejects_bad_airline() {
        let q = ListQuery {
            icao24: None,
            airline: Some("A".into()),
            military_only: false,
            q: None,
        };
        assert!(matches!(
            build_filter(&q).unwrap_err(),
            ApiError::BadRequest(_)
        ));
    }

    #[test]
    fn build_filter_defaults_are_empty() {
        let q = ListQuery {
            icao24: None,
            airline: None,
            military_only: false,
            q: None,
        };
        let f = build_filter(&q).unwrap();
        assert!(f.icao24.is_none());
        assert!(f.airline.is_none());
        assert!(!f.military_only);
        assert!(f.free_text.is_none());
    }
}
