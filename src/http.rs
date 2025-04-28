//! http module: HTTP server with Axum.

use crate::cache::RouterCache;
use crate::config::ServiceConfig;
use crate::errors::Error;
use crate::metrics::Metrics;

/// Run the HTTP server.
///
/// Serves `/healthz`, `/version`, `/metrics`, and `/:code` endpoints.
use axum::{
    body::Body,
    extract::{Extension, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Router,
};
use tokio::fs;
use std::{net::SocketAddr, time::Instant};
use prometheus::{Encoder, TextEncoder};
use serde::Deserialize;

/// Internal application state
#[derive(Clone)]
struct AppState {
    cache: RouterCache,
    metrics: Metrics,
    version: String,
}

/// Build the Axum application with routes and shared state.
fn create_app(cache: RouterCache, metrics: Metrics, version: String) -> Router<()> {
    let state = AppState { cache, metrics, version };
    Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/version", get(version_handler))
        .route("/available", get(available_handler))
        .route("/metrics", get(metrics_handler))
        .route("/", get(root_handler))
        .route("/static", get(root_handler))
        .route("/:code", get(redirect_handler))
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
    ([ (header::CONTENT_TYPE, content_type) ], buffer)
}

/// Serve the root index.html from ./static_html
async fn root_handler() -> impl IntoResponse {
    let path = "static_html/index.html";
    match fs::read_to_string(path).await {
        Ok(html) => {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(html))
                .unwrap()
        }
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap(),
    }
}

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

/// Redirect endpoint.
async fn redirect_handler(
    Extension(state): Extension<AppState>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    let start = Instant::now();
    if let Some(url) = state.cache.lookup(&code) {
        state.metrics.redirect_total.with_label_values(&[&code]).inc();
        let elapsed = start.elapsed().as_secs_f64();
        state.metrics.redirect_latency.with_label_values(&[&code]).observe(elapsed);
        Redirect::temporary(&url).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
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
    use std::collections::HashMap;
    use axum::body::Body;
    use axum::http::Request;
    use hyper::body::to_bytes;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_healthz() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "1.2.3".to_string());
        let response = app
            .clone()
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
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
            .oneshot(Request::builder().uri("/version").body(Body::empty()).unwrap())
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
            .oneshot(Request::builder().uri("/available?code=bar").body(Body::empty()).unwrap())
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
            .oneshot(Request::builder().uri("/available?code=foo").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"false");
    }
}