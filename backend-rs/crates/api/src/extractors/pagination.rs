//! Pagination query extractor.

use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use serde::Deserialize;

use flightradar_domain::ports::repositories::PageRequest;

use crate::error::ApiError;

pub const DEFAULT_PAGE_SIZE: u32 = 20;
pub const MAX_PAGE_SIZE: u32 = 200;

#[derive(Debug, Deserialize)]
struct PaginationQuery {
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Pagination(pub PageRequest);

impl Pagination {
    pub fn into_request(self) -> PageRequest {
        self.0
    }
}

#[axum::async_trait]
impl<S: Sync> FromRequestParts<S> for Pagination {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Query(q) = Query::<PaginationQuery>::try_from_uri(&parts.uri)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        let page = q.page.unwrap_or(1).max(1);
        let page_size = q
            .page_size
            .unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, MAX_PAGE_SIZE);
        Ok(Pagination(PageRequest { page, page_size }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    fn parts(uri: &str) -> Parts {
        Request::builder().uri(uri).body(()).unwrap().into_parts().0
    }

    #[tokio::test]
    async fn defaults_when_no_params() {
        let mut p = parts("/x");
        let pag = Pagination::from_request_parts(&mut p, &()).await.unwrap();
        assert_eq!(pag.0.page, 1);
        assert_eq!(pag.0.page_size, DEFAULT_PAGE_SIZE);
    }

    #[tokio::test]
    async fn parses_query_params() {
        let mut p = parts("/x?page=3&page_size=50");
        let pag = Pagination::from_request_parts(&mut p, &()).await.unwrap();
        assert_eq!(pag.0.page, 3);
        assert_eq!(pag.0.page_size, 50);
    }

    #[tokio::test]
    async fn clamps_to_max_page_size() {
        let mut p = parts("/x?page_size=9999");
        let pag = Pagination::from_request_parts(&mut p, &()).await.unwrap();
        assert_eq!(pag.0.page_size, MAX_PAGE_SIZE);
    }

    #[tokio::test]
    async fn coerces_zero_page_to_one() {
        let mut p = parts("/x?page=0");
        let pag = Pagination::from_request_parts(&mut p, &()).await.unwrap();
        assert_eq!(pag.0.page, 1);
    }

    #[tokio::test]
    async fn rejects_non_numeric_params() {
        let mut p = parts("/x?page=foo");
        assert!(matches!(
            Pagination::from_request_parts(&mut p, &())
                .await
                .unwrap_err(),
            ApiError::BadRequest(_)
        ));
    }
}
