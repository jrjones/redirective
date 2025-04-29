#!/bin/bash
set -e

# Authenticate (will prompt if needed)
eval "$(op signin)"

# Fetch the secret
SECRET=$(op read "op://Private/github.com/LINKS_REPO_TOKEN")

# Export it
export LINKS_REPO_TOKEN="$SECRET"

# (Optional) confirm
echo "LINKS_REPO_TOKEN is set."
