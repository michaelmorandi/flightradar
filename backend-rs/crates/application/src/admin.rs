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

/// Tri-state patch for an aircraft record:
/// - `None` → leave the field alone
/// - `Some(None)` → clear the field (admin explicitly removed the value)
/// - `Some(Some("x"))` → set the field to the given value
///
/// On the wire this maps to:
/// - field absent → leave alone
/// - field: null → clear
/// - field: "" → clear (after trimming)
/// - field: "x" → set
///
/// The old `Option<String>` couldn't distinguish "leave" from "clear",
/// so the admin editor had no way to remove an obsolete value.
#[derive(Debug, Clone, Default)]
#[allow(clippy::option_option)] // tri-state on purpose; see doc above
pub struct AircraftPatch {
    pub registration: Option<Option<String>>,
    pub type_code: Option<Option<String>>,
    pub type_description: Option<Option<String>>,
    pub operator: Option<Option<String>>,
    pub designator: Option<Option<String>>,
}

impl AircraftPatch {
    /// Trim & collapse whitespace-only `Some(Some(_))` values to
    /// `Some(None)` (an explicit clear). `None` and pre-cleared
    /// `Some(None)` are preserved.
    #[allow(clippy::option_option)] // tri-state, intentional
    fn normalise(value: Option<Option<String>>) -> Option<Option<String>> {
        value.map(|inner| {
            inner.and_then(|s| {
                let trimmed = s.trim().to_owned();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
        })
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

/// Apply a tri-state patch field to the matching aircraft field.
#[allow(clippy::option_option)] // tri-state, intentional
fn apply_patch_field(dst: &mut Option<String>, patch: Option<Option<String>>) {
    if let Some(value) = patch {
        *dst = value;
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

        apply_patch_field(&mut current.registration, patch.registration);
        apply_patch_field(&mut current.type_code, patch.type_code);
        apply_patch_field(&mut current.type_description, patch.type_description);
        apply_patch_field(&mut current.operator, patch.operator);
        apply_patch_field(&mut current.designator, patch.designator);
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
        async fn upsert(&self, f: &Flight) -> RepoResult<FlightId> {
            Ok(f.id.clone())
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
            registration: Some(Some("HB-JCS".into())),
            type_code: Some(Some("A320".into())),
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
            registration: Some(Some("NEW-REG".into())),
            operator: Some(Some("Swiss".into())),
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
    async fn whitespace_set_value_is_treated_as_clear() {
        // Old behaviour: whitespace-only meant "leave alone" so the
        // admin couldn't clear a field. New behaviour: whitespace
        // collapses to an explicit clear, distinguishable from the
        // "field absent" leave-alone case.
        let ac_repo = Arc::new(InMemAircraft::default());
        let mut existing = Aircraft::new(icao());
        existing.registration = Some("OLD".into());
        ac_repo.0.lock().unwrap().insert("ABCDEF".into(), existing);

        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo);
        let patch = AircraftPatch {
            registration: Some(Some("   ".into())),
            type_code: Some(Some(String::new())),
            ..Default::default()
        };
        let res = svc.update_aircraft(&icao(), patch).await.unwrap();
        assert!(res.registration.is_none(), "whitespace must clear field");
        assert!(res.type_code.is_none());
    }

    #[tokio::test]
    async fn explicit_null_clears_field() {
        let ac_repo = Arc::new(InMemAircraft::default());
        let mut existing = Aircraft::new(icao());
        existing.registration = Some("OLD".into());
        existing.type_code = Some("A320".into());
        ac_repo.0.lock().unwrap().insert("ABCDEF".into(), existing);

        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo);
        let patch = AircraftPatch {
            registration: Some(None),
            // `type_code: None` (default) — leave alone.
            ..Default::default()
        };
        let res = svc.update_aircraft(&icao(), patch).await.unwrap();
        assert!(res.registration.is_none(), "explicit null clears");
        assert_eq!(res.type_code.as_deref(), Some("A320"));
    }

    #[tokio::test]
    async fn absent_field_leaves_existing_alone() {
        let ac_repo = Arc::new(InMemAircraft::default());
        let mut existing = Aircraft::new(icao());
        existing.registration = Some("KEEP".into());
        ac_repo.0.lock().unwrap().insert("ABCDEF".into(), existing);

        let svc = AdminService::new(Arc::new(CountingFlightRepo::default()), ac_repo);
        // Empty patch — should be a no-op for every field.
        let res = svc
            .update_aircraft(&icao(), AircraftPatch::default())
            .await
            .unwrap();
        assert_eq!(res.registration.as_deref(), Some("KEEP"));
    }
}
