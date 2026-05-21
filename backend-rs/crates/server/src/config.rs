//! Typed configuration loaded from the environment.
//!
//! Env-only on purpose: matches the legacy Python deploy story, plays
//! nicely with Docker/Kubernetes secrets, and there's nothing here worth
//! the complexity of a YAML/TOML layer. Required fields fail-fast on
//! startup with a clear error.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarKind {
    Dump1090,
    Grpc,
}

impl RadarKind {
    pub fn parse(raw: &str) -> Result<Self, ConfigError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "dump1090" | "dmp1090" | "vrs" => Ok(RadarKind::Dump1090),
            "grpc" | "adsb-grpc" => Ok(RadarKind::Grpc),
            other => Err(ConfigError::InvalidValue(
                "RADAR_KIND".into(),
                other.to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    // Server
    pub bind_addr: String,
    pub allowed_origins: Vec<String>,

    // Mongo
    pub mongo_uri: String,
    pub mongo_db: String,

    // Radar
    pub radar_kind: RadarKind,
    pub radar_endpoint: String,
    pub flush_interval: Duration,
    pub position_ttl: Duration,
    pub military_only: bool,

    // Metadata
    pub nighthawk_base_url: Option<String>,

    // Reference data
    pub airlines_file: Option<PathBuf>,

    // Auth
    pub jwt_secret: String,
    pub cookie_key: Option<String>, // hex, 64 bytes if present; else generated on boot
    pub token_lifetime: Duration,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,

    // Crawler
    pub crawler_enabled: bool,
    pub crawler_interval: Duration,

    // Build metadata
    pub build_commit: String,
    pub build_timestamp: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required env var {0}")]
    Missing(String),

    #[error("invalid value for {0}: {1}")]
    InvalidValue(String, String),
}

impl Config {
    /// Load from `std::env`. Required vars: `MONGO_URI`, `MONGO_DB`,
    /// `RADAR_ENDPOINT`, `JWT_SECRET`. Everything else has a default.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_provider(&|k| std::env::var(k).ok())
    }

    /// Test seam: read from any function that maps var name → value.
    pub fn from_provider(get: &dyn Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mongo_uri = required(get, "MONGO_URI")?;
        let mongo_db = required(get, "MONGO_DB")?;
        let radar_endpoint = required(get, "RADAR_ENDPOINT")?;
        let jwt_secret = required(get, "JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err(ConfigError::InvalidValue(
                "JWT_SECRET".into(),
                "must be at least 32 characters".into(),
            ));
        }
        let radar_kind = get("RADAR_KIND")
            .as_deref()
            .map(RadarKind::parse)
            .transpose()?
            .unwrap_or(RadarKind::Dump1090);

        Ok(Self {
            bind_addr: get("BIND_ADDR").unwrap_or_else(|| "0.0.0.0:8083".into()),
            allowed_origins: parse_origins(get("ALLOWED_ORIGINS").as_deref()),
            mongo_uri,
            mongo_db,
            radar_kind,
            radar_endpoint,
            flush_interval: duration_secs(get, "FLUSH_INTERVAL_SECS", 2)?,
            position_ttl: duration_secs(get, "POSITION_TTL_SECS", 60)?,
            military_only: bool_var(get, "MILITARY_ONLY", false)?,
            nighthawk_base_url: get("NIGHTHAWK_BASE_URL"),
            airlines_file: get("AIRLINES_FILE").map(PathBuf::from),
            jwt_secret,
            cookie_key: get("COOKIE_KEY"),
            token_lifetime: duration_secs(get, "TOKEN_LIFETIME_SECS", 900)?,
            admin_email: get("ADMIN_EMAIL"),
            admin_password: get("ADMIN_PASSWORD"),
            crawler_enabled: bool_var(get, "CRAWLER_ENABLED", false)?,
            crawler_interval: duration_secs(get, "CRAWLER_INTERVAL_SECS", 20)?,
            build_commit: get("BUILD_COMMIT").unwrap_or_else(|| "unknown".into()),
            build_timestamp: get("BUILD_TIMESTAMP").unwrap_or_else(|| "unknown".into()),
        })
    }
}

fn required(get: &dyn Fn(&str) -> Option<String>, key: &str) -> Result<String, ConfigError> {
    get(key)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| ConfigError::Missing(key.into()))
}

fn duration_secs(
    get: &dyn Fn(&str) -> Option<String>,
    key: &str,
    default: u64,
) -> Result<Duration, ConfigError> {
    let Some(value) = get(key) else {
        return Ok(Duration::from_secs(default));
    };
    let parsed: u64 = value
        .parse()
        .map_err(|_| ConfigError::InvalidValue(key.into(), value.clone()))?;
    Ok(Duration::from_secs(parsed))
}

fn bool_var(
    get: &dyn Fn(&str) -> Option<String>,
    key: &str,
    default: bool,
) -> Result<bool, ConfigError> {
    let Some(v) = get(key) else {
        return Ok(default);
    };
    match v.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        other => Err(ConfigError::InvalidValue(key.into(), other.to_owned())),
    }
}

fn parse_origins(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else {
        return vec!["http://localhost:5173".into()];
    };
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn provider(entries: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = entries
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn required_vars() -> Vec<(&'static str, &'static str)> {
        vec![
            ("MONGO_URI", "mongodb://localhost:27017"),
            ("MONGO_DB", "flightradar"),
            ("RADAR_ENDPOINT", "http://localhost:8080"),
            ("JWT_SECRET", "this-is-a-32-byte-test-secret!12"),
        ]
    }

    #[test]
    fn from_provider_with_defaults() {
        let p = provider(&required_vars());
        let cfg = Config::from_provider(&p).unwrap();
        assert_eq!(cfg.radar_kind, RadarKind::Dump1090);
        assert_eq!(cfg.flush_interval, Duration::from_secs(2));
        assert_eq!(cfg.position_ttl, Duration::from_secs(60));
        assert!(!cfg.military_only);
        assert!(!cfg.crawler_enabled);
        assert_eq!(cfg.bind_addr, "0.0.0.0:8083");
    }

    #[test]
    fn missing_required_returns_error() {
        let mut vars = required_vars();
        vars.retain(|(k, _)| *k != "MONGO_URI");
        let p = provider(&vars);
        let err = Config::from_provider(&p).unwrap_err();
        assert!(matches!(err, ConfigError::Missing(ref k) if k == "MONGO_URI"));
    }

    #[test]
    fn short_jwt_secret_rejected() {
        let mut vars = required_vars();
        for (k, v) in &mut vars {
            if *k == "JWT_SECRET" {
                *v = "too-short";
            }
        }
        let p = provider(&vars);
        let err = Config::from_provider(&p).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue(ref k, _) if k == "JWT_SECRET"));
    }

    #[test]
    fn radar_kind_grpc_is_recognised() {
        let mut vars = required_vars();
        vars.push(("RADAR_KIND", "grpc"));
        let p = provider(&vars);
        let cfg = Config::from_provider(&p).unwrap();
        assert_eq!(cfg.radar_kind, RadarKind::Grpc);
    }

    #[test]
    fn radar_kind_unknown_rejected() {
        let mut vars = required_vars();
        vars.push(("RADAR_KIND", "wireless-telegraphy"));
        let p = provider(&vars);
        let err = Config::from_provider(&p).unwrap_err();
        assert!(matches!(err, ConfigError::InvalidValue(_, _)));
    }

    #[test]
    fn bool_var_accepts_common_truthy_falsy_values() {
        for v in ["1", "true", "TRUE", "yes", "ON"] {
            let mut vars = required_vars();
            vars.push(("MILITARY_ONLY", v));
            let p = provider(&vars);
            assert!(Config::from_provider(&p).unwrap().military_only);
        }
        for v in ["0", "false", "no", "OFF"] {
            let mut vars = required_vars();
            vars.push(("MILITARY_ONLY", v));
            let p = provider(&vars);
            assert!(!Config::from_provider(&p).unwrap().military_only);
        }
    }

    #[test]
    fn bool_var_rejects_garbage() {
        let mut vars = required_vars();
        vars.push(("MILITARY_ONLY", "maybe"));
        let p = provider(&vars);
        assert!(matches!(
            Config::from_provider(&p).unwrap_err(),
            ConfigError::InvalidValue(_, _)
        ));
    }

    #[test]
    fn duration_secs_accepts_number_and_rejects_text() {
        let mut vars = required_vars();
        vars.push(("FLUSH_INTERVAL_SECS", "5"));
        let p = provider(&vars);
        assert_eq!(
            Config::from_provider(&p).unwrap().flush_interval,
            Duration::from_secs(5)
        );

        let mut vars = required_vars();
        vars.push(("FLUSH_INTERVAL_SECS", "every-now-and-then"));
        let p = provider(&vars);
        assert!(matches!(
            Config::from_provider(&p).unwrap_err(),
            ConfigError::InvalidValue(_, _)
        ));
    }

    #[test]
    fn allowed_origins_parses_comma_separated() {
        let mut vars = required_vars();
        vars.push(("ALLOWED_ORIGINS", "https://a.com, https://b.com,"));
        let p = provider(&vars);
        let cfg = Config::from_provider(&p).unwrap();
        assert_eq!(
            cfg.allowed_origins,
            vec!["https://a.com".to_string(), "https://b.com".to_string()]
        );
    }

    #[test]
    fn allowed_origins_default_is_local_dev() {
        let p = provider(&required_vars());
        let cfg = Config::from_provider(&p).unwrap();
        assert_eq!(
            cfg.allowed_origins,
            vec!["http://localhost:5173".to_string()]
        );
    }

    #[test]
    fn build_metadata_falls_back_to_unknown() {
        let p = provider(&required_vars());
        let cfg = Config::from_provider(&p).unwrap();
        assert_eq!(cfg.build_commit, "unknown");
        assert_eq!(cfg.build_timestamp, "unknown");
    }
}
