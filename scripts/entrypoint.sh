#!/usr/bin/env sh
set -eu

# Install deploy key if provided and configure SSH for git
if [ -f /run/secrets/links_deploy_key ]; then
  mkdir -p /root/.ssh
  cp /run/secrets/links_deploy_key /root/.ssh/id_ed25519
  chmod 600 /root/.ssh/id_ed25519
  export GIT_SSH_COMMAND="ssh -i /root/.ssh/id_ed25519 -o IdentitiesOnly=yes"
fi

# Execute the redirective binary
exec /usr/local/bin/redirective "$@"