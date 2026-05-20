//! Airline read-side queries.

use std::sync::Arc;

use flightradar_domain::ports::airline_directory::AirlineDirectory;
use flightradar_domain::{Airline, AirlineIcao};

use crate::error::ApplicationError;

#[derive(Debug, Clone, Copy)]
pub struct AirlineQueryConfig {
    pub default_limit: u32,
    pub max_limit: u32,
}

impl Default for AirlineQueryConfig {
    fn default() -> Self {
        Self {
            default_limit: 20,
            max_limit: 100,
        }
    }
}

#[derive(Debug)]
pub struct AirlineQuery {
    directory: Arc<dyn AirlineDirectory>,
    config: AirlineQueryConfig,
}

impl AirlineQuery {
    pub fn new(directory: Arc<dyn AirlineDirectory>) -> Self {
        Self {
            directory,
            config: AirlineQueryConfig::default(),
        }
    }

    pub fn with_config(directory: Arc<dyn AirlineDirectory>, config: AirlineQueryConfig) -> Self {
        Self { directory, config }
    }

    pub async fn get(&self, icao: &AirlineIcao) -> Result<Airline, ApplicationError> {
        self.directory
            .find(icao)
            .await
            .map_err(ApplicationError::from)?
            .ok_or(ApplicationError::NotFound)
    }

    pub async fn search(
        &self,
        query: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Airline>, ApplicationError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Err(ApplicationError::InvalidInput(
                "search query must be non-empty".into(),
            ));
        }
        let limit = limit
            .unwrap_or(self.config.default_limit)
            .min(self.config.max_limit);
        Ok(self.directory.search(trimmed, limit).await?)
    }

    pub async fn all(&self) -> Result<Vec<Airline>, ApplicationError> {
        Ok(self.directory.all().await?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    use async_trait::async_trait;

    use flightradar_domain::ports::airline_directory::AirlineDirectoryError;

    use super::*;

    #[derive(Debug, Default)]
    struct StubDirectory {
        by_icao: StdMutex<HashMap<String, Airline>>,
        search_limit_seen: StdMutex<Option<u32>>,
    }
    impl StubDirectory {
        fn seed(&self, a: Airline) {
            self.by_icao.lock().unwrap().insert(a.icao.to_string(), a);
        }
    }
    #[async_trait]
    impl AirlineDirectory for StubDirectory {
        async fn find(&self, icao: &AirlineIcao) -> Result<Option<Airline>, AirlineDirectoryError> {
            Ok(self.by_icao.lock().unwrap().get(&icao.to_string()).cloned())
        }
        async fn search(
            &self,
            _query: &str,
            limit: u32,
        ) -> Result<Vec<Airline>, AirlineDirectoryError> {
            *self.search_limit_seen.lock().unwrap() = Some(limit);
            Ok(self.by_icao.lock().unwrap().values().cloned().collect())
        }
        async fn all(&self) -> Result<Vec<Airline>, AirlineDirectoryError> {
            Ok(self.by_icao.lock().unwrap().values().cloned().collect())
        }
    }

    fn airline(icao: &str, name: &str) -> Airline {
        Airline::new(AirlineIcao::new(icao).unwrap(), name)
    }

    #[tokio::test]
    async fn get_returns_known_airline() {
        let dir = Arc::new(StubDirectory::default());
        dir.seed(airline("AFR", "Air France"));
        let q = AirlineQuery::new(dir);
        let a = q.get(&AirlineIcao::new("AFR").unwrap()).await.unwrap();
        assert_eq!(a.name, "Air France");
    }

    #[tokio::test]
    async fn get_returns_not_found() {
        let q = AirlineQuery::new(Arc::new(StubDirectory::default()));
        let err = q.get(&AirlineIcao::new("XYZ").unwrap()).await.unwrap_err();
        assert!(matches!(err, ApplicationError::NotFound));
    }

    #[tokio::test]
    async fn search_rejects_empty_query() {
        let q = AirlineQuery::new(Arc::new(StubDirectory::default()));
        assert!(matches!(
            q.search("   ", None).await.unwrap_err(),
            ApplicationError::InvalidInput(_)
        ));
    }

    #[tokio::test]
    async fn search_uses_default_limit() {
        let dir = Arc::new(StubDirectory::default());
        let q = AirlineQuery::new(dir.clone());
        q.search("a", None).await.unwrap();
        assert_eq!(*dir.search_limit_seen.lock().unwrap(), Some(20));
    }

    #[tokio::test]
    async fn search_caps_at_max_limit() {
        let dir = Arc::new(StubDirectory::default());
        let q = AirlineQuery::with_config(
            dir.clone(),
            AirlineQueryConfig {
                default_limit: 20,
                max_limit: 50,
            },
        );
        q.search("a", Some(9_999)).await.unwrap();
        assert_eq!(*dir.search_limit_seen.lock().unwrap(), Some(50));
    }

    #[tokio::test]
    async fn all_returns_directory_contents() {
        let dir = Arc::new(StubDirectory::default());
        dir.seed(airline("AFR", "Air France"));
        dir.seed(airline("BAW", "British Airways"));
        let q = AirlineQuery::new(dir);
        assert_eq!(q.all().await.unwrap().len(), 2);
    }
}
