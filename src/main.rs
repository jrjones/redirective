// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! Main entry point for the redirective service.
//!
//! See `.codex/prd.md` and `.codex/architecture.md` for design docs.

mod cache;
mod config;
mod errors;
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
    // Write out shortcodes list for client-side autocomplete
    {
        // Collect and sort shortcode keys
        let mut codes: Vec<String> = config.links.keys().cloned().collect();
        codes.sort();
        let content = codes.join("\n");
        if let Err(e) = std::fs::write("static_html/shortcodes.txt", content) {
            tracing::error!("failed to write static_html/shortcodes.txt: {}", e);
        }
    }
    let cache = RouterCache::new(config.links);

    // Initialize metrics.
    let metrics = metrics::init_metrics();
    
    // Run the HTTP server.
    http::run_http_server(cache, metrics, config.service).await?;

    Ok(())
}
