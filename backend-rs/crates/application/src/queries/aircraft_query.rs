//! Aircraft read-side queries.

use std::sync::Arc;

use flightradar_domain::ports::repositories::AircraftRepository;
use flightradar_domain::{Aircraft, Icao24};

use crate::error::ApplicationError;

#[derive(Debug, Clone, Copy)]
pub struct AircraftQueryConfig {
    pub max_bulk_size: usize,
}

impl Default for AircraftQueryConfig {
    fn default() -> Self {
        Self { max_bulk_size: 50 }
    }
}

#[derive(Debug)]
pub struct AircraftQuery {
    repo: Arc<dyn AircraftRepository>,
    config: AircraftQueryConfig,
}

impl AircraftQuery {
    pub fn new(repo: Arc<dyn AircraftRepository>) -> Self {
        Self {
            repo,
            config: AircraftQueryConfig::default(),
        }
    }

    pub fn with_config(repo: Arc<dyn AircraftRepository>, config: AircraftQueryConfig) -> Self {
        Self { repo, config }
    }

    pub async fn get(&self, icao24: &Icao24) -> Result<Aircraft, ApplicationError> {
        self.repo
            .find(icao24)
            .await
            .map_err(ApplicationError::from)?
            .ok_or(ApplicationError::NotFound)
    }

    pub async fn get_many(&self, icao24s: &[Icao24]) -> Result<Vec<Aircraft>, ApplicationError> {
        if icao24s.len() > self.config.max_bulk_size {
            return Err(ApplicationError::InvalidInput(format!(
                "bulk lookup limit is {}, got {}",
                self.config.max_bulk_size,
                icao24s.len()
            )));
        }
        Ok(self.repo.find_many(icao24s).await?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use flightradar_domain::ports::repositories::RepoResult;

    use super::*;

    #[derive(Debug, Default)]
    struct StubRepo {
        by_icao: StdMutex<HashMap<String, Aircraft>>,
    }
    impl StubRepo {
        fn seed(&self, ac: Aircraft) {
            self.by_icao
                .lock()
                .unwrap()
                .insert(ac.icao24.to_string(), ac);
        }
    }
    #[async_trait]
    impl AircraftRepository for StubRepo {
        async fn find(&self, icao24: &Icao24) -> RepoResult<Option<Aircraft>> {
            Ok(self
                .by_icao
                .lock()
                .unwrap()
                .get(&icao24.to_string())
                .cloned())
        }
        async fn find_many(&self, icao24s: &[Icao24]) -> RepoResult<Vec<Aircraft>> {
            let map = self.by_icao.lock().unwrap();
            Ok(icao24s
                .iter()
                .filter_map(|i| map.get(&i.to_string()).cloned())
                .collect())
        }
        async fn upsert(&self, ac: &Aircraft) -> RepoResult<()> {
            self.by_icao
                .lock()
                .unwrap()
                .insert(ac.icao24.to_string(), ac.clone());
            Ok(())
        }
    }

    fn ac(icao: &str) -> Aircraft {
        Aircraft::new(Icao24::new(icao).unwrap())
    }

    #[tokio::test]
    async fn get_returns_known_aircraft() {
        let repo = Arc::new(StubRepo::default());
        repo.seed(ac("ABCDEF"));
        let q = AircraftQuery::new(repo);
        let out = q.get(&Icao24::new("ABCDEF").unwrap()).await.unwrap();
        assert_eq!(out.icao24.as_str(), "ABCDEF");
    }

    #[tokio::test]
    async fn get_returns_not_found() {
        let repo = Arc::new(StubRepo::default());
        let q = AircraftQuery::new(repo);
        let err = q.get(&Icao24::new("000001").unwrap()).await.unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[tokio::test]
    async fn get_many_returns_known_subset() {
        let repo = Arc::new(StubRepo::default());
        repo.seed(ac("ABCDEF"));
        repo.seed(ac("123456"));
        let q = AircraftQuery::new(repo);

        let icaos = vec![
            Icao24::new("ABCDEF").unwrap(),
            Icao24::new("123456").unwrap(),
            Icao24::new("000001").unwrap(),
        ];
        let out = q.get_many(&icaos).await.unwrap();
        assert_eq!(out.len(), 2);
    }

    #[tokio::test]
    async fn get_many_enforces_bulk_limit() {
        let repo = Arc::new(StubRepo::default());
        let q = AircraftQuery::with_config(repo, AircraftQueryConfig { max_bulk_size: 2 });
        let icaos = vec![
            Icao24::new("ABCDEF").unwrap(),
            Icao24::new("123456").unwrap(),
            Icao24::new("000001").unwrap(),
        ];
        let err = q.get_many(&icaos).await.unwrap_err();
        assert!(matches!(err, ApplicationError::InvalidInput(_)));
    }
}
