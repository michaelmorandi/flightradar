//! Admin-only use cases.
//!
//! Kept small on purpose — only the operations the dashboard genuinely
//! needs (stats + per-aircraft edit). The richer crawler-control surface
//! from the legacy Python admin is deliberately dropped; the cron-style
//! crawler is configured via env and runs autonomously.

use std::sync::Arc;

use flightradar_domain::ports::repositories::{
    AircraftRepository, FlightFilter, FlightRepository, PageRequest,
};
use flightradar_domain::{Aircraft, Icao24};

use crate::error::ApplicationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminStats {
    pub flight_count: u64,
}

#[derive(Debug, Clone, Default)]
pub struct AircraftPatch {
    pub registration: Option<String>,
    pub type_code: Option<String>,
    pub type_description: Option<String>,
    pub operator: Option<String>,
    pub designator: Option<String>,
}

impl AircraftPatch {
    fn normalise(value: Option<String>) -> Option<String> {
        value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
    }

    #[must_use]
    pub fn into_normalised(self) -> Self {
        Self {
            registration: Self::normalise(self.registration),
            type_code: Self::normalise(self.type_code),
            type_description: Self::normalise(self.type_description),
            operator: Self::normalise(self.operator),
            designator: Self::normalise(self.designator),
        }
    }
}

#[derive(Debug)]
pub struct AdminService {
    flights: Arc<dyn FlightRepository>,
    aircraft: Arc<dyn AircraftRepository>,
}

impl AdminService {
    pub fn new(flights: Arc<dyn FlightRepository>, aircraft: Arc<dyn AircraftRepository>) -> Self {
        Self { flights, aircraft }
    }

    pub async fn stats(&self) -> Result<AdminStats, ApplicationError> {
        // Reuse list() to derive the count. page_size=1 is enough because
        // we only read the `total` field on the returned page.
        let page = self
            .flights
            .list(
                &FlightFilter::default(),
                PageRequest {
                    page: 1,
                    page_size: 1,
                },
            )
            .await?;
        Ok(AdminStats {
            flight_count: page.total,
        })
    }

    /// Upsert-with-merge: load the existing record (if any), overwrite
    /// only the fields the admin actually set, persist. `None`/empty
    /// strings in the patch leave the existing value alone.
    pub async fn update_aircraft(
        &self,
        icao24: &Icao24,
        patch: AircraftPatch,
    ) -> Result<Aircraft, ApplicationError> {
        let patch = patch.into_normalised();
        let mut current = self
            .aircraft
            .find(icao24)
            .await?
            .unwrap_or_else(|| Aircraft::new(icao24.clone()));

        if let Some(v) = patch.registration {
            current.registration = Some(v);
        }
        if let Some(v) = patch.type_code {
            current.type_code = Some(v);
        }
        if let Some(v) = patch.type_description {
            current.type_description = Some(v);
        }
        if let Some(v) = patch.operator {
            current.operator = Some(v);
        }
        if let Some(v) = patch.designator {
            current.designator = Some(v);
        }
        // Mark the source so it's clear edits came from the dashboard.
        current.source = Some(flightradar_domain::AircraftSource::new("admin"));

        self.aircraft.upsert(&current).await?;
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use flightradar_domain::ports::repositories::{Page, PageRequest, RepoResult, RepositoryError};
    use flightradar_domain::{Flight, FlightId};

    use super::*;

    #[derive(Debug, Default)]
    struct CountingFlightRepo {
        total: u64,
    }
    #[async_trait]
    impl FlightRepository for CountingFlightRepo {
        async fn upsert(&self, _f: &Flight) -> RepoResult<()> {
            Ok(())
        }
        async fn find_by_id(&self, _id: &FlightId) -> RepoResult<Flight> {
            Err(RepositoryError::NotFound)
        }
        async fn find_open_for_icao24(&self, _icao24: &Icao24) -> RepoResult<Option<Flight>> {
            Ok(None)
        }
        async fn list(&self, _f: &FlightFilter, page: PageRequest) -> RepoResult<Page<Flight>> {
            Ok(Page {
                items: vec![],
                total: self.total,
                page: page.page,
                page_size: page.page_size,
            })
        }
    }

    #[derive(Debug, Default)]
    struct InMemAircraft(StdMutex<HashMap<String, Aircraft>>);
    #[async_trait]
    impl AircraftRepository for InMemAircraft {
        async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
            Ok(self.0.lock().unwrap().get(&icao24.to_string()).cloned())
        }
        async fn find_many(&self, _icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
            Ok(vec![])
        }
        async fn upsert(&self, ac: &Aircraft) -> RepoResult<()> {
            self.0
                .lock()
                .unwrap()
                .insert(ac.icao24.to_string(), ac.clone());
            Ok(())
        }
    }

    fn icao() -> Icao24 {
        Icao24::new("ABCDEF").unwrap()
    }

    #[tokio::test]
    async fn stats_returns_flight_total() {
        let svc = AdminService::new(
            Arc::new(CountingFlightRepo { total: 42 }),
            Arc::new(InMemAircraft::default()),
        );
        let s = svc.stats().await.unwrap();
        assert_eq!(s.flight_count, 42);
    }

    #[tokio::test]
    async fn update_creates_new_aircraft_when_missing() {
        let ac_repo = Arc::new(InMemAircraft::default());
        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo.clone());

        let patch = AircraftPatch {
            registration: Some("HB-JCS".into()),
            type_code: Some("A320".into()),
            ..Default::default()
        };
        let res = svc.update_aircraft(&icao(), patch).await.unwrap();
        assert_eq!(res.icao24, icao());
        assert_eq!(res.registration.as_deref(), Some("HB-JCS"));
        assert_eq!(res.type_code.as_deref(), Some("A320"));
        assert_eq!(res.source.as_ref().unwrap().as_str(), "admin");

        let stored = ac_repo.0.lock().unwrap().get("ABCDEF").cloned().unwrap();
        assert_eq!(stored, res);
    }

    #[tokio::test]
    async fn update_merges_into_existing_aircraft() {
        let ac_repo = Arc::new(InMemAircraft::default());
        let mut existing = Aircraft::new(icao());
        existing.registration = Some("OLD-REG".into());
        existing.type_code = Some("A320".into());
        ac_repo.0.lock().unwrap().insert("ABCDEF".into(), existing);

        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo.clone());
        let patch = AircraftPatch {
            registration: Some("NEW-REG".into()),
            operator: Some("Swiss".into()),
            ..Default::default()
        };
        let res = svc.update_aircraft(&icao(), patch).await.unwrap();
        // Edited fields replaced…
        assert_eq!(res.registration.as_deref(), Some("NEW-REG"));
        assert_eq!(res.operator.as_deref(), Some("Swiss"));
        // …untouched fields preserved.
        assert_eq!(res.type_code.as_deref(), Some("A320"));
    }

    #[tokio::test]
    async fn empty_strings_in_patch_are_treated_as_absent() {
        let ac_repo = Arc::new(InMemAircraft::default());
        let mut existing = Aircraft::new(icao());
        existing.registration = Some("OLD".into());
        ac_repo.0.lock().unwrap().insert("ABCDEF".into(), existing);

        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo);
        let patch = AircraftPatch {
            registration: Some("   ".into()),
            type_code: Some(String::new()),
            ..Default::default()
        };
        let res = svc.update_aircraft(&icao(), patch).await.unwrap();
        // Whitespace + empty strings get normalised to None → no overwrite.
        assert_eq!(res.registration.as_deref(), Some("OLD"));
        assert!(res.type_code.is_none());
    }
}
