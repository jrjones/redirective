//! Main entry point for the redirective service.
//!
//! See `.codex/prd.md` and `.codex/architecture.md` for design docs.

mod cache;
mod config;
mod errors;
mod git_sync;
mod http;
mod metrics;

use crate::cache::RouterCache;
use crate::config::Config;
use crate::errors::Error;

/// Application entry point.
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing subscriber (JSON output, env filter).
    tracing_subscriber::FmtSubscriber::builder()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Load configuration (links.yaml and service settings).
    let config = Config::load("links.yaml")?;
    let cache = RouterCache::new(config.links);

    // Initialize metrics.
    let metrics = metrics::init_metrics();

    // Start Git sync background task.
    git_sync::start_git_sync(
        "links.yaml",
        cache.clone(),
        config.service.reload_interval_secs,
        metrics.clone(),
    );

    // Run the HTTP server.
    http::run_http_server(cache, metrics, config.service).await?;

    Ok(())
}
