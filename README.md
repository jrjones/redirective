# Redirective
Redirective is a stateless Rust micro-service that maps short codes to full URLs using an in-memory table sourced from a Git-backed YAML file. It exposes three HTTP endpoints:
 - `GET /{code}`: 302 redirect to the target URL if found, otherwise 404.
 - `GET /healthz`: health check endpoint.
 - `GET /version`: service version endpoint.

Features:
 - Thread-safe, lock-free reads with `ArcSwap`.
 - Hot-reload of mappings on file changes via Git sync daemon.
 - Prometheus metrics and structured JSON logging.
 
## Configuration
 - `links.yaml`: contains mappings from codes to URLs. Example provided.
 - `redirective.toml`: service settings (bind address, reload interval, TLS paths).
 
## Development
## Utilities

### Exporting from YOURLS

If you have an existing [YOURLS](https://yourls.org) instance, you can export its short URL mappings into a `links.yaml` file compatible with Redirective:

```bash
# Set database connection environment variables:
export YOURLS_DB_HOST=your-db-host
export YOURLS_DB_USER=your-db-user
export YOURLS_DB_PASS=your-db-password
export YOURLS_DB_NAME=your-db-name
# Optional: port (defaults to 3306)
export YOURLS_DB_PORT=3306

# Run the export script to generate links.yaml
scripts/export_yourls.sh > links.yaml
```

This creates a YAML file where each key is the YOURLS keyword and the value is the target URL, ready to be consumed by Redirective.
 ### Prerequisites
 - Rust 1.74+ toolchain
 - Git
 ### Building
 ```bash
 cargo build --release
 ```
 ### Running
 ```bash
 ./target/release/redirective
 ```
 ### Docker
 Build and run via Docker:
 ```bash
 # Build image with private links repository (requires LINKS_REPO_TOKEN env)
 docker build --build-arg LINKS_REPO_TOKEN=$LINKS_REPO_TOKEN -t redirective:latest .
 # Run the service
 docker run -p 8080:8080 redirective:latest
 ```
 ## CI
 This project uses GitHub Actions for CI. See `.github/workflows/ci.yml`.
 ## Documentation
 - [Architecture](.codex/architecture.md)
 - [PRD](.codex/prd.md)
