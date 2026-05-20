//! Composition root for the flightradar backend.
//!
//! Wiring of ports → adapters → use cases → HTTP layer will land here in
//! subsequent commits. For now this is a placeholder so the workspace
//! produces a binary target.

#[allow(clippy::unnecessary_wraps)] // real wiring (below) will be fallible
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!("flightradar-server skeleton (no service wired yet)");
    Ok(())
}
