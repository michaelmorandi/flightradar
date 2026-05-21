use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;

use flightradar_api::{build_router, middleware::MiddlewareConfig};
use flightradar_server::composition::build_production_deps;
use flightradar_server::observability::{init as init_observability, ObservabilityConfig};
use flightradar_server::supervisor::{spawn_supervised, SupervisorConfig};
use flightradar_server::{build_app, Config};

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env().context("load config")?;

    let _guard = init_observability(&ObservabilityConfig::from_env(config.build_commit.clone()))?;

    tracing::info!(
        commit = %config.build_commit,
        bind = %config.bind_addr,
        radar = ?config.radar_kind,
        crawler = config.crawler_enabled,
        "flightradar-backend starting"
    );

    let deps = build_production_deps(&config).await?;
    let app = build_app(&config, deps).await?;

    let middleware_config = MiddlewareConfig {
        allowed_origins: config.allowed_origins.clone(),
        request_timeout: std::time::Duration::from_secs(30),
    };
    let router = build_router(app.state.clone(), &middleware_config);

    // --- Supervised background tasks --------------------------------------
    let updater = app.flight_updater.clone();
    let flush_interval = config.flush_interval;
    let _updater_handle =
        spawn_supervised("flight_updater", SupervisorConfig::default(), move || {
            let updater = updater.clone();
            async move {
                if let Err(err) = updater.run(flush_interval).await {
                    tracing::warn!(error = %err, "flight updater exited with error");
                }
            }
        });

    if let Some(crawler) = app.aircraft_crawler.clone() {
        let interval = config.crawler_interval;
        let _crawler_handle =
            spawn_supervised("aircraft_crawler", SupervisorConfig::default(), move || {
                let crawler = crawler.clone();
                let interval = interval;
                async move {
                    let mut ticker = tokio::time::interval(interval);
                    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    loop {
                        ticker.tick().await;
                        match crawler.run_once().await {
                            Ok(report) => {
                                tracing::debug!(?report, "crawler tick complete");
                            }
                            Err(err) => {
                                tracing::warn!(error = %err, "crawler tick failed");
                            }
                        }
                    }
                }
            });
    }

    // --- HTTP server ------------------------------------------------------
    let listener = TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("bind {}", config.bind_addr))?;
    tracing::info!(addr = %config.bind_addr, "listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum serve")?;
    tracing::info!("shutdown complete");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}

// Silence the `Arc` import warning when this is compiled as a binary with
// the full feature set — kept here so we can grow main without juggling imports.
#[allow(dead_code)]
fn _phantom(_: Arc<()>) {}
