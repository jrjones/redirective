# High‑Level Architecture -- "Redirective" (Rust URL Redirector)

## 1. Overview

A stateless Rust micro‑service that maps `/{code}` to full URLs using an in‑memory table sourced from a Git‑backed YAML file. It exposes only three HTTP endpoints (`/{code}`, `/healthz`, `/version`) and publishes Prometheus metrics. The binary is <5 MB, runs anywhere, and scales horizontally behind any load balancer.

## 2. Runtime Components

| Component | Responsibility | Key Crate / Tech | 
| ---- | ---- | ----  |
| **HTTP Server** | Handles requests, performs 302 redirect, exposes health/version | `axum` + `hyper` | 
| **Router Cache** | Thread‑safe `Arc<HashMap<String, String>>` providing O(1) lookup | `dashmap` or `ArcSwap` | 
| **Config Loader** | Parses `links.yaml` into map, validates schema | `serde_yaml` | 
| **Git Sync Daemon** | Periodically `git pull --ff-only`; triggers hot reload on diff | `git2`, `tokio` task | 
| **Hot‑Reload Manager** | Atomically swaps map to avoid blocking readers | `ArcSwap` or RCU pattern | 
| **Metrics & Logging** | Prometheus counters/histograms; JSON structured logs | `prometheus`, `tracing`, `tracing_subscriber` | 

## 3. Request & Reload Flow
    
sequenceDiagram
```
    client->>Redirective: GET /foo
    Redirective->>Router Cache: lookup("foo")
    alt found
        Redirective-->>client: 302 https://target
    else not found
        Redirective-->>client: 404
    end
 ``````   
    
    
sequenceDiagram
``````
    Git Sync Daemon->>Origin Repo: git pull
    Note over Git Sync Daemon: if links.yaml changed
    Git Sync Daemon->>Config Loader: parse / validate
    Config Loader-->>Hot‑Reload Manager: new map
    Hot‑Reload Manager->>Router Cache: atomic swap
    Hot‑Reload Manager-->>Prometheus: reload_success++
 ```   

## 4. Concurrency & Hot Reload Strategy

- **Data Structure**: `ArcSwap<HashMap>` allows lock‑free reads; swap is ~1 µs.
- **Reload Path**: validation occurs off‑line; swap executes only when new map is fully built.
- **Zero Downtime**: readers never block; worst‑case they serve stale map during the micro‑second swap.

## 5. Configuration Management

- **Repo Layout**: `links.yaml` at repo root; optional `redirective.toml` for service settings (reload interval, port, etc.).
- **CI Guardrails**: YAML schema check + unit test in PR; prevents bad deploys.
- **Deploy Key**: read‑only SSH key baked into container.

## 6. Deployment & Scaling
- **Container Image**: `FROM gcr.io/distroless/cc` with static binary.
- **Kubernetes**: HPA on `redirects_per_second`; ConfigMap mounts repo path (initial clone) or init‑container clone.
- **Load Balancer**: TLS termination (ALB/Nginx/Envoy). Can also run bare‑metal with `rustls`.
- **Stateless**: any replica can be killed/restarted without data loss.

## 7. Observability
- **Prometheus**: `redirect_total{code}`, `redirect_latency_seconds`, `reload_fail_total`.
- **Tracing**: `TRACE_ID` per request for correlation.
- **Alerting**: 5xx rate >0.1%, reload failures, P99 >1 ms.

## 8. Security & Compliance
- **Transport**: TLS 1.3 only.
- **Headers**: `Strict‑Transport‑Security`, `X‑Content‑Type‑Options`, `X‑Frame‑Options`.
- **Supply Chain**: Dependabot + Cargo audit.
- **SAST**: Clippy + RustSec in CI.

## 9. Dependencies & Build Pipeline
1. `cargo build --release` → static binary.
2. `docker build` → multi‑stage, strip symbols.
3. GitHub Actions: test, audit, container scan, push to registry.

## 10. Future Extensions (Not in MVP)

- Multi‑tenant namespaces (`/{team}/{code}`).
- Signed redirect tokens for one‑time links.
- Web UI with analytics.
- S3‑backed `links.yaml` for environments without Git.

_Open questions_: query‑string passthrough rules, default redirect when code missing, acceptable reload interval default (e.g., 60 s vs webhook).
