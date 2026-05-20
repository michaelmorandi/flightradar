//! Mongo client and database handles.

use std::time::Duration;

use mongodb::options::ClientOptions;
use mongodb::{Client, Database};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct MongoConfig {
    pub uri: String,
    pub database: String,
    pub app_name: Option<String>,
    pub connect_timeout: Duration,
    pub server_selection_timeout: Duration,
}

impl MongoConfig {
    pub fn new(uri: impl Into<String>, database: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            database: database.into(),
            app_name: Some("flightradar".to_string()),
            connect_timeout: Duration::from_secs(10),
            server_selection_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Error)]
pub enum MongoConnectionError {
    #[error("invalid mongo URI: {0}")]
    InvalidUri(String),

    #[error("mongo client failure: {0}")]
    Client(#[source] mongodb::error::Error),
}

#[derive(Debug, Clone)]
pub struct MongoConnection {
    client: Client,
    db: Database,
}

impl MongoConnection {
    pub async fn connect(config: &MongoConfig) -> Result<Self, MongoConnectionError> {
        let mut opts = ClientOptions::parse(&config.uri)
            .await
            .map_err(|e| MongoConnectionError::InvalidUri(e.to_string()))?;
        opts.app_name.clone_from(&config.app_name);
        opts.connect_timeout = Some(config.connect_timeout);
        opts.server_selection_timeout = Some(config.server_selection_timeout);

        let client = Client::with_options(opts).map_err(MongoConnectionError::Client)?;
        let db = client.database(&config.database);
        Ok(Self { client, db })
    }

    pub fn database(&self) -> &Database {
        &self.db
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_have_sensible_timeouts() {
        let cfg = MongoConfig::new("mongodb://localhost:27017", "flightradar");
        assert_eq!(cfg.database, "flightradar");
        assert_eq!(cfg.app_name.as_deref(), Some("flightradar"));
        assert!(cfg.connect_timeout >= Duration::from_secs(5));
        assert!(cfg.server_selection_timeout >= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn connect_with_invalid_uri_returns_invalid_uri() {
        let cfg = MongoConfig::new("not-a-mongo-uri", "flightradar");
        let err = MongoConnection::connect(&cfg).await.unwrap_err();
        assert!(matches!(err, MongoConnectionError::InvalidUri(_)));
    }
}
