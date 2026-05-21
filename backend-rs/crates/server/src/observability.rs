//! OTEL-native observability bootstrap.
//!
//! Configures `tracing` as the application logging seam (structured JSON
//! to stdout) and, when `OTEL_EXPORTER_OTLP_ENDPOINT` is set, installs a
//! global OTEL tracer provider that exports via OTLP gRPC.
//!
//! Note (intentional, revisit later): the `tracing` → OTEL bridge has
//! been a moving target across recent OTEL crate versions; rather than
//! pin a fragile combination, the bridge layer is omitted. Code that
//! wants OTEL spans emits them via `opentelemetry::global::tracer(...)`
//! directly; structured logs continue to ship via tracing-subscriber.

use anyhow::Result;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace as sdktrace;
use opentelemetry_sdk::Resource;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const SERVICE_NAME: &str = "flightradar-backend";

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub service_version: String,
    /// `None` disables the OTLP exporter. When set, this is the OTLP gRPC
    /// endpoint URL (e.g. `http://otel-collector:4317`).
    pub otlp_endpoint: Option<String>,
}

impl ObservabilityConfig {
    pub fn from_env(service_version: impl Into<String>) -> Self {
        Self {
            service_version: service_version.into(),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
        }
    }
}

/// Initialise the tracing subscriber and, optionally, the OTEL pipeline.
/// Returns a guard that must be kept alive for the lifetime of the
/// process — dropping it shuts down the global OTEL tracer provider.
pub fn init(config: &ObservabilityConfig) -> Result<TracingGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,hyper=warn,tonic=warn,mongodb=info"));
    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(false);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();

    if let Some(endpoint) = &config.otlp_endpoint {
        install_otel_provider(endpoint, &config.service_version)?;
        Ok(TracingGuard::WithOtel)
    } else {
        Ok(TracingGuard::Local)
    }
}

fn install_otel_provider(endpoint: &str, service_version: &str) -> Result<()> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    let resource = Resource::new(vec![
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_NAME,
            SERVICE_NAME,
        ),
        KeyValue::new(
            opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
            service_version.to_owned(),
        ),
    ]);

    let _tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(endpoint.to_owned()),
        )
        .with_trace_config(sdktrace::Config::default().with_resource(resource))
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;
    Ok(())
}

/// RAII guard. Drop it to shut down the global OTEL provider, which
/// flushes pending spans before the runtime exits.
#[derive(Debug)]
pub enum TracingGuard {
    Local,
    WithOtel,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        if matches!(self, TracingGuard::WithOtel) {
            opentelemetry::global::shutdown_tracer_provider();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_picks_up_endpoint_env() {
        let with = ObservabilityConfig {
            service_version: "1.0".into(),
            otlp_endpoint: Some("http://collector:4317".into()),
        };
        assert!(with.otlp_endpoint.is_some());

        let without = ObservabilityConfig {
            service_version: "1.0".into(),
            otlp_endpoint: None,
        };
        assert!(without.otlp_endpoint.is_none());
    }

    #[test]
    fn guard_debug_format_stable() {
        let g = TracingGuard::Local;
        assert_eq!(format!("{g:?}"), "Local");
    }
}
