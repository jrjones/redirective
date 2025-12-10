#!/usr/bin/env sh
set -eu

LINKS_REPO="git@github.com:jrjones/redirective-links.git"

# Install deploy key if provided and configure SSH for git
if [ -f /run/secrets/links_deploy_key ]; then
  mkdir -p /root/.ssh
  cp /run/secrets/links_deploy_key /root/.ssh/id_ed25519
  chmod 600 /root/.ssh/id_ed25519
  # Add GitHub to known hosts to avoid interactive prompt
  ssh-keyscan -t ed25519 github.com >> /root/.ssh/known_hosts 2>/dev/null || true
  export GIT_SSH_COMMAND="ssh -i /root/.ssh/id_ed25519 -o IdentitiesOnly=yes -o StrictHostKeyChecking=no"

  # Clone the links repo into /app if not already a git repo
  # This makes /app the redirective-links repo so git pull works
  if [ ! -d /app/.git ]; then
    echo "Initializing git repo for links..."
    cd /app
    git init
    git remote add origin "$LINKS_REPO"
    git fetch origin main
    # Remove bundled links.yaml to allow checkout, then checkout main tracking origin
    rm -f links.yaml
    git checkout -b main origin/main
    echo "Links repository initialized."
  fi
fi

# Execute the redirective binary
exec /usr/local/bin/redirective "$@"