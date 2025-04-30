# Redirective
Redirective is a stateless Rust micro-service that maps short codes to full URLs using an in-memory table sourced from a Git-backed YAML file. It exposes three HTTP endpoints:
 - `GET /{code}`: 302 redirect to the target URL if found, otherwise 404.
 - `GET /healthz`: health check endpoint.
 - `GET /version`: service version endpoint.
 - `GET /available?code=foobar`: Returns true or false based on whether or not the code passed in the query string is available

If none of the above match, it will look for a matching file or directory in the `static_html` folder, allowing you to host a static site.

404s will redirect to /index.html for now (plan to add a 404.html in a future release.)

## Codex
This project was done in part because I was using [Yourls](https://github.com/YOURLS/YOURLS), and it's a little old and bloated. But mostly as a fun project to try out [OpenAI's Codex CLI](https://github.com/openai/codex) - to see how I'm getting the best results from it, check out the files in the .codex directory, which I reference frequently in my prompts. Fun stuff. :) I have probably written barely more than half of the code here. 

## Features:
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
docker build -t redirective:latest .
docker run -p 8080:8080 redirective:latest
```  
// Or, to hot-reload against a local clone:
// git clone https://github.com/jrjones/redirective-links.git ./redirective-links
// docker run -v $PWD/redirective-links:/app -w /app -p 8080:8080 redirective:latest
 ## CI
 This project uses GitHub Actions for CI. See `.github/workflows/ci.yml`.
 ## Documentation
 - [Architecture](.codex/architecture.md)
 - [PRD](.codex/prd.md)
