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

/// A client is considered stale, and evicted from the rate limiter map, once
/// its day window has been idle this long (i.e. it hasn't made a request in
/// over a day).
const RATE_LIMIT_DAY_WINDOW: Duration = Duration::from_secs(60 * 60 * 24);

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
        // Evict stale entries for other clients before growing the map.
        // `ip` itself is kept regardless so the window-reset logic below can
        // run on it normally.
        clients.retain(|&other, info| {
            other == ip || now.duration_since(info.day_window_start) < RATE_LIMIT_DAY_WINDOW
        });
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
        if now.duration_since(info.day_window_start) >= RATE_LIMIT_DAY_WINDOW {
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

/// Build the Axum application with routes and shared state. `reload_mutex`
/// is shared with the background poll task (see `spawn_poll_task`) so the
/// two reload triggers never race each other's `git pull`.
fn create_app(
    cache: RouterCache,
    metrics: Metrics,
    version: String,
    service: ServiceConfig,
    reload_mutex: Arc<TokioMutex<()>>,
) -> Router<()> {
    let state = AppState {
        cache,
        metrics,
        version,
        reload_mutex,
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

/// Returns true if `peer` is loopback or an RFC1918/unique-local private
/// address, i.e. it can only be our own nginx sitting in front of us.
///
/// There's no explicit trusted-proxy list in this codebase; nginx always
/// reaches us over the docker bridge or loopback, so "peer is private" is an
/// adequate proxy for "peer is our nginx" without adding a config knob.
fn is_trusted_proxy(peer: IpAddr) -> bool {
    match peer {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => {
            if v6.is_loopback() {
                true
            } else if let Some(mapped) = v6.to_ipv4_mapped() {
                mapped.is_loopback() || mapped.is_private()
            } else {
                // fc00::/7 (unique local).
                (v6.segments()[0] & 0xfe00) == 0xfc00
            }
        }
    }
}

/// Extract a single IP address from a header value, treating a
/// missing/empty/unparseable value as absent.
fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// Resolve the real client IP for rate-limiting purposes.
///
/// Only trusts `X-Real-IP`/`X-Forwarded-For` when `peer` (the TCP peer) is
/// our own nginx (see `is_trusted_proxy`); a public peer means the request
/// didn't come through our proxy, so its headers could be spoofed and are
/// ignored in favor of the peer address itself. nginx sets
/// `X-Real-IP $http_cf_connecting_ip`, which is empty for non-CloudFlare
/// (tailnet) requests, so an empty header is treated the same as absent.
fn resolve_client_ip(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    if !is_trusted_proxy(peer) {
        return peer;
    }
    if let Some(ip) = header_ip(headers, "x-real-ip") {
        return ip;
    }
    let xff_first = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<IpAddr>().ok());
    if let Some(ip) = xff_first {
        return ip;
    }
    peer
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

/// Run `git rev-parse HEAD` in `repo_dir`, returning the trimmed commit hash
/// on success. Used only to detect whether a pull actually moved HEAD.
fn git_head(git_binary: &str, repo_dir: &std::path::Path) -> Option<String> {
    Command::new(git_binary)
        .current_dir(repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Run `git pull --ff-only` and, on success, reload the link table from
/// `links.yaml` into `cache`.
///
/// Returns `Some(changed)` on a successful reload, where `changed` reports
/// whether the pull actually moved HEAD (useful for quiet-steady-state
/// logging), or `None` if the pull or the reload failed. Metrics are
/// incremented here so every caller gets consistent accounting.
async fn reload_links(cache: &RouterCache, metrics: &Metrics, git_binary: &str) -> Option<bool> {
    let repo_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let before = git_head(git_binary, &repo_dir);

    let pulled = Command::new(git_binary)
        .current_dir(&repo_dir)
        .args(["pull", "--ff-only"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if !pulled {
        metrics.reload_fail.inc();
        return None;
    }

    let cfg = match Config::load("links.yaml") {
        Ok(cfg) => cfg,
        Err(_) => {
            metrics.reload_fail.inc();
            return None;
        }
    };

    let mut codes: Vec<String> = cfg.links.keys().cloned().collect();
    codes.sort();
    let _ = std::fs::write("static_html/shortcodes.txt", codes.join("\n"));
    cache.swap(cfg.links);
    metrics.reload_success.inc();

    let after = git_head(git_binary, &repo_dir);
    Some(before.is_some() && after != before)
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
    if reload_links(&cache, &metrics, git_binary).await.is_some()
        && let Some(peer_url) = relay_target
    {
        relay_to_peer(&peer_url, &metrics).await;
    }
}

/// Convert a configured poll interval into a `Duration`, treating `None` or
/// `0` as "polling disabled". Kept side-effect free so it can be unit
/// tested without touching tokio's clock.
fn poll_interval(poll_interval_secs: Option<u64>) -> Option<Duration> {
    match poll_interval_secs {
        None | Some(0) => None,
        Some(secs) => Some(Duration::from_secs(secs)),
    }
}

/// Spawn the background git-poll reload task, if `poll_interval_secs`
/// enables it. Shares `reload_mutex` with the webhook handler so a poll and
/// a webhook-triggered reload never run `git pull` concurrently.
fn spawn_poll_task(
    cache: RouterCache,
    metrics: Metrics,
    reload_mutex: Arc<TokioMutex<()>>,
    poll_interval_secs: Option<u64>,
) {
    let Some(interval) = poll_interval(poll_interval_secs) else {
        tracing::info!("git-poll reload disabled (poll_interval_secs unset or 0)");
        return;
    };
    tracing::info!(
        interval_secs = interval.as_secs(),
        "git-poll reload enabled"
    );
    task::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // The first tick fires immediately; links are already fresh from
        // startup, so skip it and only reload on subsequent ticks.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let _guard = reload_mutex.lock().await;
            match reload_links(&cache, &metrics, GIT_BINARY).await {
                Some(true) => tracing::info!("git-poll reload: links changed"),
                Some(false) => {}
                None => tracing::warn!("git-poll reload: pull or reload failed"),
            }
        }
    });
}

/// Webhook endpoint to trigger a git pull and reload.
async fn webhook_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Extension(state): Extension<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let ip = resolve_client_ip(addr.ip(), &headers);
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
    let reload_mutex = Arc::new(TokioMutex::new(()));
    spawn_poll_task(
        cache.clone(),
        metrics.clone(),
        reload_mutex.clone(),
        service.poll_interval_secs,
    );
    let app = create_app(cache, metrics, version, service.clone(), reload_mutex);
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
            poll_interval_secs: None,
        }
    }

    fn new_reload_mutex() -> Arc<TokioMutex<()>> {
        Arc::new(TokioMutex::new(()))
    }

    #[tokio::test]
    async fn test_healthz() {
        let cache = RouterCache::new(HashMap::new());
        let metrics = init_metrics();
        let app = create_app(
            cache,
            metrics,
            "1.2.3".to_string(),
            default_service(),
            new_reload_mutex(),
        );
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
        let app = create_app(
            cache,
            metrics,
            "vX.Y".to_string(),
            default_service(),
            new_reload_mutex(),
        );
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
        let app = create_app(
            cache,
            metrics,
            "1.0".to_string(),
            default_service(),
            new_reload_mutex(),
        );
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
        let app = create_app(
            cache,
            metrics,
            "1.0".to_string(),
            default_service(),
            new_reload_mutex(),
        );
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

    fn loopback_peer() -> IpAddr {
        IpAddr::from([127, 0, 0, 1])
    }

    fn public_peer() -> IpAddr {
        IpAddr::from([203, 0, 113, 5])
    }

    #[test]
    fn test_is_trusted_proxy_loopback_and_private() {
        assert!(is_trusted_proxy(loopback_peer()));
        assert!(is_trusted_proxy(IpAddr::from([10, 0, 0, 5])));
        assert!(is_trusted_proxy(IpAddr::from([172, 17, 0, 2])));
        assert!(is_trusted_proxy(IpAddr::from([192, 168, 1, 1])));
        assert!(is_trusted_proxy(std::net::Ipv6Addr::LOCALHOST.into()));
    }

    #[test]
    fn test_is_trusted_proxy_public_is_not_trusted() {
        assert!(!is_trusted_proxy(public_peer()));
    }

    #[test]
    fn test_resolve_client_ip_honors_x_real_ip_from_loopback() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.9".parse().unwrap());
        assert_eq!(
            resolve_client_ip(loopback_peer(), &headers),
            IpAddr::from([203, 0, 113, 9])
        );
    }

    #[test]
    fn test_resolve_client_ip_ignores_spoofed_header_from_public_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.9".parse().unwrap());
        // A public peer isn't our nginx, so the header must be ignored and
        // the peer address used instead - otherwise any caller could set
        // X-Real-IP and bypass the rate limiter entirely.
        assert_eq!(resolve_client_ip(public_peer(), &headers), public_peer());
    }

    #[test]
    fn test_resolve_client_ip_uses_leftmost_xff_entry() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "203.0.113.9, 10.0.0.1, 10.0.0.2".parse().unwrap(),
        );
        assert_eq!(
            resolve_client_ip(loopback_peer(), &headers),
            IpAddr::from([203, 0, 113, 9])
        );
    }

    #[test]
    fn test_resolve_client_ip_empty_header_treated_as_absent() {
        let mut headers = HeaderMap::new();
        // nginx sets X-Real-IP to $http_cf_connecting_ip, which is empty for
        // non-CloudFlare (tailnet) traffic.
        headers.insert("x-real-ip", "".parse().unwrap());
        headers.insert("x-forwarded-for", "10.0.0.7".parse().unwrap());
        assert_eq!(
            resolve_client_ip(loopback_peer(), &headers),
            IpAddr::from([10, 0, 0, 7])
        );
    }

    #[test]
    fn test_resolve_client_ip_falls_back_to_peer_when_no_headers() {
        let headers = HeaderMap::new();
        assert_eq!(
            resolve_client_ip(loopback_peer(), &headers),
            loopback_peer()
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_n_then_blocks_nplus1() {
        let limiter = RateLimiter::new(2, 100);
        let ip = IpAddr::from([192, 168, 1, 50]);
        assert!(limiter.allow(ip).await);
        assert!(limiter.allow(ip).await);
        assert!(!limiter.allow(ip).await);
    }

    #[tokio::test]
    async fn test_rate_limiter_tracks_ips_independently() {
        let limiter = RateLimiter::new(1, 100);
        let a = IpAddr::from([192, 168, 1, 1]);
        let b = IpAddr::from([192, 168, 1, 2]);
        assert!(limiter.allow(a).await);
        assert!(!limiter.allow(a).await);
        // A different IP has its own budget.
        assert!(limiter.allow(b).await);
    }

    #[tokio::test]
    async fn test_rate_limiter_evicts_stale_entries() {
        let limiter = RateLimiter::new(10, 100);
        let stale_ip = IpAddr::from([192, 168, 1, 99]);
        let now = Instant::now();
        let stale_start = now
            .checked_sub(RATE_LIMIT_DAY_WINDOW + Duration::from_secs(1))
            .expect("test process has been up over a day");
        {
            let mut clients = limiter.clients.lock().await;
            clients.insert(
                stale_ip,
                RateInfo {
                    minute_count: 5,
                    minute_window_start: stale_start,
                    day_count: 5,
                    day_window_start: stale_start,
                },
            );
        }
        let fresh_ip = IpAddr::from([192, 168, 1, 100]);
        assert!(limiter.allow(fresh_ip).await);
        let clients = limiter.clients.lock().await;
        assert!(!clients.contains_key(&stale_ip));
        assert!(clients.contains_key(&fresh_ip));
    }

    #[test]
    fn test_poll_interval_none_when_unset() {
        assert_eq!(poll_interval(None), None);
    }

    #[test]
    fn test_poll_interval_none_when_zero() {
        assert_eq!(poll_interval(Some(0)), None);
    }

    #[test]
    fn test_poll_interval_some_when_positive() {
        assert_eq!(poll_interval(Some(60)), Some(Duration::from_secs(60)));
    }
}
