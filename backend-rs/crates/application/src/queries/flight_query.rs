//! Flight read-side queries.

use std::sync::Arc;

use flightradar_domain::ports::repositories::{
    FlightFilter, FlightRepository, Page, PageRequest, PositionRepository,
};
use flightradar_domain::{Flight, FlightId, LiveSnapshot, PositionReport};

use crate::error::ApplicationError;
use crate::live_state::LiveState;

#[derive(Debug)]
pub struct FlightQuery {
    flights: Arc<dyn FlightRepository>,
    positions: Arc<dyn PositionRepository>,
    live: LiveState,
}

impl FlightQuery {
    pub fn new(
        flights: Arc<dyn FlightRepository>,
        positions: Arc<dyn PositionRepository>,
        live: LiveState,
    ) -> Self {
        Self {
            flights,
            positions,
            live,
        }
    }

    pub async fn list(
        &self,
        filter: &FlightFilter,
        page: PageRequest,
    ) -> Result<Page<Flight>, ApplicationError> {
        Ok(self.flights.list(filter, page).await?)
    }

    pub async fn get(&self, id: &FlightId) -> Result<Flight, ApplicationError> {
        self.flights.find_by_id(id).await.map_err(|err| match err {
            flightradar_domain::ports::repositories::RepositoryError::NotFound => {
                ApplicationError::NotFound
            }
            other => other.into(),
        })
    }

    pub async fn history(&self, id: &FlightId) -> Result<Vec<PositionReport>, ApplicationError> {
        Ok(self.positions.history(id).await?)
    }

    /// Lock-free read of the current live picture.
    pub fn live(&self) -> Arc<LiveSnapshot> {
        self.live.read()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::float_cmp)] // intentional: positions are bit-identical
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;
    use time::OffsetDateTime;

    use flightradar_domain::ports::repositories::{RepoResult, RepositoryError};
    use flightradar_domain::{AirlineIcao, Callsign, Icao24, LivePosition};

    use super::*;

    fn t(secs: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_700_000_000 + secs).unwrap()
    }

    fn flight(id: &str, icao: &str) -> Flight {
        Flight {
            id: FlightId::new(id),
            icao24: Icao24::new(icao).unwrap(),
            callsign: Some(Callsign::new("AFR990").unwrap()),
            airline_icao: Some(AirlineIcao::new("AFR").unwrap()),
            is_military: false,
            first_contact: t(0),
            last_contact: t(60),
        }
    }

    #[derive(Debug, Default)]
    struct StubFlightRepo {
        flights: StdMutex<HashMap<String, Flight>>,
        list_response: StdMutex<Vec<Flight>>,
    }
    #[async_trait]
    impl FlightRepository for StubFlightRepo {
        async fn upsert(&self, _f: &Flight) -> RepoResult<()> {
            Ok(())
        }
        async fn find_by_id(&self, id: &FlightId) -> RepoResult<Flight> {
            self.flights
                .lock()
                .unwrap()
                .get(id.as_str())
                .cloned()
                .ok_or(RepositoryError::NotFound)
        }
        async fn find_open_for_icao24(&self, _icao24: &Icao24) -> RepoResult<Option<Flight>> {
            Ok(None)
        }
        async fn list(
            &self,
            _filter: &FlightFilter,
            page: PageRequest,
        ) -> RepoResult<Page<Flight>> {
            let items = self.list_response.lock().unwrap().clone();
            let total = items.len() as u64;
            Ok(Page {
                items,
                total,
                page: page.page,
                page_size: page.page_size,
            })
        }
    }

    #[derive(Debug, Default)]
    struct StubPositionRepo {
        history_response: StdMutex<Vec<PositionReport>>,
    }
    #[async_trait]
    impl PositionRepository for StubPositionRepo {
        async fn append(&self, _id: &FlightId, _p: &PositionReport) -> RepoResult<()> {
            Ok(())
        }
        async fn append_batch(&self, _e: &[(FlightId, PositionReport)]) -> RepoResult<()> {
            Ok(())
        }
        async fn history(&self, _id: &FlightId) -> RepoResult<Vec<PositionReport>> {
            Ok(self.history_response.lock().unwrap().clone())
        }
    }

    #[tokio::test]
    async fn list_returns_page() {
        let flights = Arc::new(StubFlightRepo::default());
        flights
            .list_response
            .lock()
            .unwrap()
            .push(flight("f1", "ABCDEF"));
        let q = FlightQuery::new(
            flights,
            Arc::new(StubPositionRepo::default()),
            LiveState::empty(),
        );
        let page = q
            .list(
                &FlightFilter::default(),
                PageRequest {
                    page: 1,
                    page_size: 10,
                },
            )
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.total, 1);
    }

    #[tokio::test]
    async fn get_returns_flight_when_present() {
        let flights = Arc::new(StubFlightRepo::default());
        flights
            .flights
            .lock()
            .unwrap()
            .insert("f1".into(), flight("f1", "ABCDEF"));
        let q = FlightQuery::new(
            flights,
            Arc::new(StubPositionRepo::default()),
            LiveState::empty(),
        );
        let f = q.get(&FlightId::new("f1")).await.unwrap();
        assert_eq!(f.id.as_str(), "f1");
    }

    #[tokio::test]
    async fn get_returns_not_found() {
        let q = FlightQuery::new(
            Arc::new(StubFlightRepo::default()),
            Arc::new(StubPositionRepo::default()),
            LiveState::empty(),
        );
        let err = q.get(&FlightId::new("missing")).await.unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[tokio::test]
    async fn history_returns_positions() {
        let positions = Arc::new(StubPositionRepo::default());
        positions
            .history_response
            .lock()
            .unwrap()
            .push(PositionReport::new(Icao24::new("ABCDEF").unwrap(), 1.0, 2.0, t(0)).unwrap());
        let q = FlightQuery::new(
            Arc::new(StubFlightRepo::default()),
            positions,
            LiveState::empty(),
        );
        let hist = q.history(&FlightId::new("f1")).await.unwrap();
        assert_eq!(hist.len(), 1);
    }

    #[tokio::test]
    async fn live_reads_through_to_state() {
        let live = LiveState::empty();
        let mut map = HashMap::new();
        map.insert(
            Icao24::new("ABCDEF").unwrap(),
            LivePosition {
                lat: 1.0,
                lon: 2.0,
                alt_ft: None,
                ground_speed_kt: None,
                track_deg: None,
                callsign: None,
                category: None,
                updated_at: t(0),
            },
        );
        live.publish(LiveSnapshot::new(map, t(0)));

        let q = FlightQuery::new(
            Arc::new(StubFlightRepo::default()),
            Arc::new(StubPositionRepo::default()),
            live,
        );
        assert_eq!(q.live().len(), 1);
    }
}
