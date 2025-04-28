#!/usr/bin/env bash
set -euo pipefail

# Check for docker exec fallback or mysql CLI
DB_CMD=()
if [[ -n "${YOURLS_DOCKER_CONTAINER:-}" ]]; then
    # Use mysql inside Docker container
    if ! command -v docker >/dev/null; then
        echo "Error: docker not found; cannot exec into container $YOURLS_DOCKER_CONTAINER" >&2
        exit 1
    fi
    DB_CMD=(docker exec -i "$YOURLS_DOCKER_CONTAINER" mysql)
else
    # Use local mysql client via TCP
    if ! command -v mysql >/dev/null; then
        echo "Error: mysql client not found. Install mysql-client or set YOURLS_DOCKER_CONTAINER." >&2
        exit 1
    fi
    DB_CMD=(mysql --protocol=TCP -h "$YOURLS_DB_HOST" -P "$YOURLS_DB_PORT" -u "$YOURLS_DB_USER" -p"$YOURLS_DB_PASS")
fi

# Export YOURLS mappings to a YAML file suitable for Redirective
#
# Environment variables required:
#   YOURLS_DB_HOST   - MySQL host for YOURLS database
#   YOURLS_DB_USER   - Username for YOURLS database
#   YOURLS_DB_PASS   - Password for YOURLS database
#   YOURLS_DB_NAME   - Database name for YOURLS
#   YOURLS_DB_PORT   - (Optional) port, default 3306

: "${YOURLS_DB_HOST:?Need YOURLS_DB_HOST}"
: "${YOURLS_DB_USER:?Need YOURLS_DB_USER}"
: "${YOURLS_DB_PASS:?Need YOURLS_DB_PASS}"
: "${YOURLS_DB_NAME:?Need YOURLS_DB_NAME}"
: "${YOURLS_DB_PORT:=3306}"

### Query and export
"${DB_CMD[@]}" "$YOURLS_DB_NAME" -Nse \
    "SELECT keyword, url, IFNULL(title, '') FROM yourls_url ORDER BY keyword;" |
while IFS=$'\t' read -r key url title; do
    # Output as YAML mapping, append title as comment if present
    if [[ -n "$title" ]]; then
        echo "${key}: ${url} | ${title}"
    else
        echo "${key}: ${url}"
    fi
done