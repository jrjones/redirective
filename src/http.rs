// (C) Copyright 2025, Joseph R. Jones - https://jrj.org - Licensed under MIT License
//! http module: HTTP server with Axum.

use crate::cache::RouterCache;
use crate::config::{Config, ServiceConfig};
use crate::errors::Error;
use crate::metrics::Metrics;

/// Run the HTTP server.
///
/// Serves `/healthz`, `/version`, `/metrics`, and `/:code` endpoints.
use axum::{
    Router,
    extract::{ConnectInfo, Extension, Query},
    http::{HeaderMap, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use mime_guess::from_path;
use prometheus::{Encoder, TextEncoder};
use serde::Deserialize;
use std::path::Path as FsPath;

/// Header marking a webhook request as relayed from a peer node.
/// Requests carrying this header are never relayed again (loop prevention).
const RELAY_HEADER: &str = "x-redirective-relay";

/// Path to the git binary used for reload pulls.
const GIT_BINARY: &str = "/usr/bin/git";

/// Timeout for relaying the webhook to the peer node.
const RELAY_TIMEOUT: Duration = Duration::from_secs(5);

/// Configuration for the reload webhook.
#[derive(Clone)]
struct WebhookConfig {
    path: String,
    peer_url: Option<String>,
}

/// Rate limit information per client IP.
struct RateInfo {
    minute_count: u32,
    minute_window_start: Instant,
    day_count: u32,
    day_window_start: Instant,
}

/// Simple in-memory rate limiter.
struct RateLimiter {
    clients: TokioMutex<HashMap<IpAddr, RateInfo>>,
    per_minute: u32,
    per_day: u32,
}

impl RateLimiter {
    fn new(per_minute: u32, per_day: u32) -> Self {
        RateLimiter {
            clients: TokioMutex::new(HashMap::new()),
            per_minute,
            per_day,
        }
    }

    /// Returns true if the request from `ip` is allowed.
    async fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut clients = self.clients.lock().await;
        let info = clients.entry(ip).or_insert(RateInfo {
            minute_count: 0,
            minute_window_start: now,
            day_count: 0,
            day_window_start: now,
        });
        if now.duration_since(info.minute_window_start) >= Duration::from_secs(60) {
            info.minute_window_start = now;
            info.minute_count = 0;
        }
        if now.duration_since(info.day_window_start) >= Duration::from_secs(60 * 60 * 24) {
            info.day_window_start = now;
            info.day_count = 0;
        }
        if info.minute_count + 1 > self.per_minute || info.day_count + 1 > self.per_day {
            false
        } else {
            info.minute_count += 1;
            info.day_count += 1;
            true
        }
    }
}
use std::env;
use std::process::Command;

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::fs;
use tokio::sync::Mutex as TokioMutex;
use tokio::task;

/// Internal application state
#[derive(Clone)]
struct AppState {
    cache: RouterCache,
    metrics: Metrics,
    version: String,
    reload_mutex: Arc<TokioMutex<()>>,
    rate_limiter: Arc<RateLimiter>,
    webhook_config: WebhookConfig,
}

/// Build the Axum application with routes and shared state.
fn create_app(
    cache: RouterCache,
    metrics: Metrics,
    version: String,
    service: ServiceConfig,
) -> Router<()> {
    let state = AppState {
        cache,
        metrics,
        version,
        reload_mutex: Arc::new(TokioMutex::new(())),
        rate_limiter: Arc::new(RateLimiter::new(
            service.rate_limit_per_minute,
            service.rate_limit_per_day,
        )),
        webhook_config: WebhookConfig {
            path: service.webhook_path.clone(),
            peer_url: service.peer_url.clone(),
        },
    };
    let mut router = Router::new()
        .route("/healthz", get(healthz_handler))
        .route("/version", get(version_handler))
        .route("/available", get(available_handler))
        .route("/metrics", get(metrics_handler));

    // Webhook endpoint to trigger reload (POST) and reject other methods (405)
    router = router
        .route(&state.webhook_config.path, post(webhook_handler))
        .route(
            &state.webhook_config.path,
            get(|| async { StatusCode::METHOD_NOT_ALLOWED }),
        );

    router
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
        // 302 Found redirect with Location header
        let location = [(header::LOCATION, url.clone())];
        return (StatusCode::FOUND, location).into_response();
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

/// Decide whether this webhook should be relayed to a peer.
///
/// Returns the peer URL to relay to when `peer_url` is configured and the
/// incoming request did not itself arrive via relay (no `X-Redirective-Relay`
/// header). A relayed request is never relayed again, so two nodes pointing
/// at each other cannot loop.
fn relay_target(peer_url: Option<&str>, headers: &HeaderMap) -> Option<String> {
    match peer_url {
        Some(url) if !headers.contains_key(RELAY_HEADER) => Some(url.to_string()),
        _ => None,
    }
}

/// Fire a relay POST to the peer's webhook endpoint, marking it with the
/// relay-guard header. Failures are logged and counted but never propagate.
async fn relay_to_peer(peer_url: &str, metrics: &Metrics) {
    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri(peer_url)
        .header(RELAY_HEADER, "1")
        .body(hyper::Body::empty());
    let request = match request {
        Ok(req) => req,
        Err(e) => {
            tracing::error!(peer = peer_url, error = %e, "invalid peer_url; relay skipped");
            metrics.relay_fail.inc();
            return;
        }
    };
    let client = hyper::Client::builder().build(hyper_tls::HttpsConnector::new());
    match tokio::time::timeout(RELAY_TIMEOUT, client.request(request)).await {
        Ok(Ok(resp)) if resp.status().is_success() => {
            tracing::info!(peer = peer_url, status = %resp.status(), "relayed webhook to peer");
            metrics.relay_success.inc();
        }
        Ok(Ok(resp)) => {
            tracing::warn!(peer = peer_url, status = %resp.status(), "peer relay rejected");
            metrics.relay_fail.inc();
        }
        Ok(Err(e)) => {
            tracing::warn!(peer = peer_url, error = %e, "peer relay failed");
            metrics.relay_fail.inc();
        }
        Err(_) => {
            tracing::warn!(
                peer = peer_url,
                timeout_secs = RELAY_TIMEOUT.as_secs(),
                "peer relay timed out"
            );
            metrics.relay_fail.inc();
        }
    }
}

/// Run a git pull and reload the link table, then (only on a successful
/// reload) relay the webhook to `relay_target`, if any. Relay failures do
/// not affect the reload outcome.
async fn reload_and_relay(
    cache: RouterCache,
    metrics: Metrics,
    relay_target: Option<String>,
    git_binary: &str,
) {
    let repo_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if let Ok(status) = Command::new(git_binary)
        .current_dir(&repo_dir)
        .args(["pull", "--ff-only"])
        .status()
    {
        if status.success() {
            if let Ok(cfg) = Config::load("links.yaml") {
                if env::current_dir().is_ok() {
                    let mut codes: Vec<String> = cfg.links.keys().cloned().collect();
                    codes.sort();
                    let content = codes.join("\n");
                    let _ = std::fs::write("static_html/shortcodes.txt", &content);
                }
                cache.swap(cfg.links);
                metrics.reload_success.inc();
                // Only a successful reload propagates to the peer.
                if let Some(peer_url) = relay_target {
                    relay_to_peer(&peer_url, &metrics).await;
                }
            } else {
                metrics.reload_fail.inc();
            }
        } else {
            metrics.reload_fail.inc();
        }
    } else {
        metrics.reload_fail.inc();
    }
}

/// Webhook endpoint to trigger a git pull and reload.
async fn webhook_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(state): Extension<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ip = addr.ip();
    if !state.rate_limiter.allow(ip).await {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let relay_target = relay_target(state.webhook_config.peer_url.as_deref(), &headers);
    let reload_mutex = state.reload_mutex.clone();
    let cache = state.cache.clone();
    let metrics = state.metrics.clone();
    task::spawn(async move {
        let _guard = reload_mutex.lock().await;
        reload_and_relay(cache, metrics, relay_target, GIT_BINARY).await;
    });
    StatusCode::ACCEPTED.into_response()
}

/// Run the HTTP server.
pub async fn run_http_server(
    cache: RouterCache,
    metrics: Metrics,
    service: ServiceConfig,
) -> Result<(), Error> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let app = create_app(cache, metrics, version, service.clone());
    let addr: SocketAddr = service.address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
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

    // Default ServiceConfig for tests
    fn default_service() -> ServiceConfig {
        ServiceConfig {
            address: "127.0.0.1:0".to_string(),
            webhook_path: "/git-webhook".to_string(),
            rate_limit_per_minute: 1,
            rate_limit_per_day: 100,
            peer_url: None,
        }
    }

    #[tokio::test]
    async fn test_healthz() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let app = create_app(cache, metrics, "1.2.3".to_string(), default_service());
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
        let app = create_app(cache, metrics, "vX.Y".to_string(), default_service());
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
        let app = create_app(cache, metrics, "1.0".to_string(), default_service());
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
        let app = create_app(cache, metrics, "1.0".to_string(), default_service());
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

    /// Spawn a mock peer webhook server on an ephemeral port. Returns its
    /// webhook URL and a log of the headers of every request it receives.
    async fn spawn_mock_peer() -> (String, Arc<TokioMutex<Vec<HeaderMap>>>) {
        let received: Arc<TokioMutex<Vec<HeaderMap>>> = Arc::new(TokioMutex::new(Vec::new()));
        let log = received.clone();
        let app = Router::new().route(
            "/git-webhook",
            post(move |headers: HeaderMap| {
                let log = log.clone();
                async move {
                    log.lock().await.push(headers);
                    StatusCode::ACCEPTED
                }
            }),
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app.into_make_service()),
        );
        (format!("http://{}/git-webhook", addr), received)
    }

    #[test]
    fn test_relay_target_set_when_peer_configured_and_no_guard() {
        let headers = HeaderMap::new();
        assert_eq!(
            relay_target(Some("http://peer/git-webhook"), &headers),
            Some("http://peer/git-webhook".to_string())
        );
    }

    #[test]
    fn test_relay_target_none_when_guard_header_present() {
        let mut headers = HeaderMap::new();
        headers.insert(RELAY_HEADER, "1".parse().unwrap());
        assert_eq!(
            relay_target(Some("http://peer/git-webhook"), &headers),
            None
        );
    }

    #[test]
    fn test_relay_target_none_when_peer_absent() {
        let headers = HeaderMap::new();
        assert_eq!(relay_target(None, &headers), None);
    }

    #[tokio::test]
    async fn test_relay_fires_after_successful_reload() {
        let (peer_url, received) = spawn_mock_peer().await;
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        // /usr/bin/true stands in for a git pull that succeeds.
        reload_and_relay(cache, metrics.clone(), Some(peer_url), "/usr/bin/true").await;
        assert_eq!(metrics.reload_success.get(), 1);
        assert_eq!(metrics.relay_success.get(), 1);
        assert_eq!(metrics.relay_fail.get(), 0);
        let requests = received.lock().await;
        assert_eq!(requests.len(), 1);
        // The relayed request must carry the guard header so the peer
        // does not relay it back (loop prevention).
        assert_eq!(requests[0].get(RELAY_HEADER).unwrap(), "1");
    }

    #[tokio::test]
    async fn test_no_relay_when_target_none() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        reload_and_relay(cache, metrics.clone(), None, "/usr/bin/true").await;
        assert_eq!(metrics.reload_success.get(), 1);
        assert_eq!(metrics.relay_success.get(), 0);
        assert_eq!(metrics.relay_fail.get(), 0);
    }

    #[tokio::test]
    async fn test_no_relay_on_failed_reload() {
        let (peer_url, received) = spawn_mock_peer().await;
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        // /usr/bin/false stands in for a git pull that fails.
        reload_and_relay(cache, metrics.clone(), Some(peer_url), "/usr/bin/false").await;
        assert_eq!(metrics.reload_fail.get(), 1);
        assert_eq!(metrics.relay_success.get(), 0);
        assert_eq!(metrics.relay_fail.get(), 0);
        assert!(received.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_relay_failure_counted_but_reload_still_succeeds() {
        // Bind then drop a listener to get a port with nothing listening.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let peer_url = format!("http://{}/git-webhook", addr);
        reload_and_relay(cache, metrics.clone(), Some(peer_url), "/usr/bin/true").await;
        assert_eq!(metrics.reload_success.get(), 1);
        assert_eq!(metrics.relay_success.get(), 0);
        assert_eq!(metrics.relay_fail.get(), 1);
    }
}
