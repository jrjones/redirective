#!/usr/bin/env bash
set -euo pipefail
# Parse options: allow -t to set LINKS_REPO_TOKEN
while getopts "t:" opt; do
  case "$opt" in
    t) export LINKS_REPO_TOKEN="$OPTARG" ;;
    *) echo "Usage: $0 [-t token]" >&2; exit 1 ;;
  esac
done
shift $((OPTIND -1))

echo "Stopping running redirective containers (by image)..."
# Stop any containers running the redirective image
containers=$(docker ps --filter ancestor=redirective:latest -q)
if [ -n "$containers" ]; then
  docker stop $containers
fi

echo "Removing redirective images..."
# Remove any images named redirective
images=$(docker images redirective -q)
if [ -n "$images" ]; then
  docker rmi -f $images
fi

if [ -z "${LINKS_REPO_TOKEN:-}" ]; then
  echo "Sourcing 1passwordexport to set LINKS_REPO_TOKEN..."
  source ./scripts/1passExport.sh
fi

: "${LINKS_REPO_TOKEN:?Environment variable LINKS_REPO_TOKEN must be set}"

echo "Cloning redirective-links repository..."
rm -rf redirective-links
git clone https://$LINKS_REPO_TOKEN@github.com/jrjones/redirective-links.git redirective-links

echo "Building redirective Docker image..."
docker build -t redirective:latest .

echo "Starting redirective container..."
docker run --rm \
  -v "$PWD/redirective-links:/app" \
  -v "$PWD/static_html:/app/static_html" \
  -w /app \
  -p 8080:8080 \
  redirective:latest
