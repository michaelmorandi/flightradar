//! Health + info endpoints.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::state::{AppState, BuildInfo};

#[derive(Debug, Serialize)]
pub struct InfoResponse {
    pub commit: String,
    pub build_timestamp: String,
}

impl From<BuildInfo> for InfoResponse {
    fn from(b: BuildInfo) -> Self {
        Self {
            commit: b.commit,
            build_timestamp: b.build_timestamp,
        }
    }
}

pub async fn alive() -> StatusCode {
    StatusCode::OK
}

pub async fn ready() -> StatusCode {
    // Real readiness would probe Mongo / radar source. Kept simple here;
    // the supervisor task in `server` is the source of truth.
    StatusCode::OK
}

pub async fn info(State(state): State<AppState>) -> Json<InfoResponse> {
    Json(state.build.into())
}
