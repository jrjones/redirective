//! http module: HTTP server with Axum.

use crate::cache::RouterCache;
use crate::config::ServiceConfig;
use crate::errors::Error;
use crate::metrics::Metrics;

/// Run the HTTP server.
///
/// Serves `/healthz`, `/version`, `/metrics`, and `/:code` endpoints.
use axum::{
    Router,
    extract::{Extension, Query},
    http::{Uri, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use mime_guess::from_path;
use prometheus::{Encoder, TextEncoder};
use serde::Deserialize;
use std::path::Path as FsPath;
use std::{net::SocketAddr, time::Instant};
use tokio::fs;

/// Internal application state
#[derive(Clone)]
struct AppState {
    cache: RouterCache,
    metrics: Metrics,
    version: String,
}

/// Build the Axum application with routes and shared state.
fn create_app(cache: RouterCache, metrics: Metrics, version: String) -> Router<()> {
    let state = AppState {
        cache,
        metrics,
        version,
    };
    Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/version", get(version_handler))
        .route("/available", get(available_handler))
        .route("/metrics", get(metrics_handler))
        // Fallback to handle redirects, static files, or SPA index.html
        .fallback(spa_handler)
        .layer(Extension(state))
}

/// Health check endpoint.
async fn healthz_handler() -> &'static str {
    "OK"
}

/// Version endpoint.
async fn version_handler(Extension(state): Extension<AppState>) -> String {
    state.version.clone()
}

/// Metrics endpoint.
async fn metrics_handler(Extension(state): Extension<AppState>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    // Copy content type to owned String
    let content_type = encoder.format_type().to_string();
    ([(header::CONTENT_TYPE, content_type)], buffer)
}

// root_handler removed; spa_handler fallback handles static files and index.html

/// Available endpoint: tells whether a shortcode is unused.
#[derive(Deserialize)]
struct AvailableParams {
    /// The shortcode to check.
    code: String,
}

async fn available_handler(
    Extension(state): Extension<AppState>,
    Query(params): Query<AvailableParams>,
) -> impl IntoResponse {
    if state.cache.lookup(&params.code).is_none() {
        "true"
    } else {
        "false"
    }
}

// redirect_handler removed; use spa_handler fallback for redirects and static files

/// SPA/static fallback: tries shortcode redirect, then static files, else serves index.html
async fn spa_handler(Extension(state): Extension<AppState>, uri: Uri) -> Response {
    let start = Instant::now();
    let raw_path = uri.path();
    let trimmed = raw_path.trim_start_matches('/');
    // shortcode redirect
    if let Some(url) = state.cache.lookup(trimmed) {
        state
            .metrics
            .redirect_total
            .with_label_values(&[trimmed])
            .inc();
        let elapsed = start.elapsed().as_secs_f64();
        state
            .metrics
            .redirect_latency
            .with_label_values(&[trimmed])
            .observe(elapsed);
        return Redirect::temporary(&url).into_response();
    }
    // static file or directory
    let file_rel = if trimmed.is_empty() {
        "index.html"
    } else {
        trimmed
    };
    let fs_path = FsPath::new("static_html").join(file_rel);
    if let Ok(meta) = fs::metadata(&fs_path).await {
        if meta.is_file() {
            if let Ok(contents) = fs::read(&fs_path).await {
                let mime = from_path(&fs_path).first_or_octet_stream();
                return ([(header::CONTENT_TYPE, mime.to_string())], contents).into_response();
            }
        } else if meta.is_dir() {
            let idx = fs_path.join("index.html");
            if let Ok(contents) = fs::read(&idx).await {
                return (
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())],
                    contents,
                )
                    .into_response();
            }
        }
    }
    // fallback to root index.html
    let default = fs::read("static_html/index.html").await.unwrap_or_default();
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8".to_string())],
        default,
    )
        .into_response()
}

/// Run the HTTP server.
pub async fn run_http_server(
    cache: RouterCache,
    metrics: Metrics,
    service: ServiceConfig,
) -> Result<(), Error> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let app = create_app(cache, metrics, version);
    let addr: SocketAddr = service.address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::init_metrics;
    use axum::body::Body;
    use axum::http::Request;
    use axum::http::StatusCode;
    use hyper::body::to_bytes;
    use std::collections::HashMap;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_healthz() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "1.2.3".to_string());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"OK");
    }

    #[tokio::test]
    async fn test_version() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "vX.Y".to_string());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/version")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"vX.Y");
    }

    #[tokio::test]
    async fn test_available_unused() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), "http://example.com".to_string());
        let cache = RouterCache::new(map);
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "1.0".to_string());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/available?code=bar")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"true");
    }

    #[tokio::test]
    async fn test_available_used() {
        let mut map = HashMap::new();
        map.insert("foo".to_string(), "http://example.com".to_string());
        let cache = RouterCache::new(map);
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "1.0".to_string());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/available?code=foo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"false");
    }
}
