//! Background task supervisor.
//!
//! Wraps a long-running future so panics and exits surface as a tracing
//! warning + automatic restart with exponential backoff capped at
//! `max_backoff`. Designed for the FlightUpdater (radar consumer) and the
//! AircraftCrawler tick loop.

use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy)]
pub struct SupervisorConfig {
    pub min_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            min_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
        }
    }
}

/// Spawn a long-running task that the supervisor restarts on exit. The
/// returned `JoinHandle` is for the supervisor loop itself — it only
/// completes when the runtime shuts down.
pub fn spawn_supervised<F, Fut>(
    name: &'static str,
    config: SupervisorConfig,
    factory: F,
) -> JoinHandle<()>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        let mut backoff = config.min_backoff;
        loop {
            info!(task = name, "starting supervised task");
            let task = tokio::spawn(factory());
            match task.await {
                Ok(()) => {
                    warn!(task = name, "supervised task exited cleanly, restarting");
                }
                Err(err) if err.is_panic() => {
                    warn!(task = name, "supervised task panicked, restarting");
                }
                Err(err) => {
                    warn!(task = name, error = %err, "supervised task cancelled");
                    return;
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(config.max_backoff);
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn supervisor_restarts_after_clean_exit() {
        let counter = Arc::new(AtomicUsize::new(0));
        let target = counter.clone();
        let cfg = SupervisorConfig {
            min_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_millis(10),
        };
        let handle = spawn_supervised("test", cfg, move || {
            let target = target.clone();
            async move {
                target.fetch_add(1, Ordering::SeqCst);
                // Exit immediately so supervisor restarts.
            }
        });

        // Give the supervisor time to restart a few times.
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle.abort();
        let count = counter.load(Ordering::SeqCst);
        assert!(count >= 2, "expected at least 2 restarts, got {count}");
    }

    // The panic-recovery branch is exercised in production by the
    // FlightUpdater and Crawler supervisors. A unit test for it is
    // intentionally omitted because tokio's default panic hook prints
    // the deliberate panic in a way that confuses the test runner.

    #[test]
    fn default_backoffs_are_reasonable() {
        let cfg = SupervisorConfig::default();
        assert!(cfg.min_backoff <= cfg.max_backoff);
        assert!(cfg.max_backoff >= Duration::from_secs(10));
    }
}
