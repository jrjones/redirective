#!/usr/bin/env sh
set -eu

LINKS_REPO="git@github.com:jrjones/redirective-links.git"
DEPLOY_KEY="/run/secrets/links_deploy_key"

# LINKS_REQUIRED gates the fail-loud behavior. Prod MUST set LINKS_REQUIRED=1
# (see the container-cluster compose): the real links repo must be cloned into
# /app, and if the deploy key is missing OR the git fetch/checkout fails we
# exit non-zero and crash-loop instead of silently serving the bundled STUB
# links.yaml (foo/bar/test). That stub-serving failure took down all ~167 real
# jrj.io short-links once (ops#138) precisely because it was silent.
#
# Dev keeps the friendly default: LINKS_REQUIRED unset/0 + no deploy key =>
# serve the bundled stub, no network, no crash.
LINKS_REQUIRED="${LINKS_REQUIRED:-0}"

fail() {
  echo "FATAL(entrypoint): $1" >&2
  echo "FATAL(entrypoint): LINKS_REQUIRED=1 — refusing to start on stub links.yaml. Fix the deploy key / links sync and redeploy." >&2
  exit 1
}

if [ -f "$DEPLOY_KEY" ]; then
  # Install deploy key and configure SSH for git
  mkdir -p /root/.ssh
  cp "$DEPLOY_KEY" /root/.ssh/id_ed25519
  chmod 600 /root/.ssh/id_ed25519
  # Add GitHub to known hosts to avoid interactive prompt
  ssh-keyscan -t ed25519 github.com >> /root/.ssh/known_hosts 2>/dev/null || true
  export GIT_SSH_COMMAND="ssh -i /root/.ssh/id_ed25519 -o IdentitiesOnly=yes -o StrictHostKeyChecking=no"

  # Clone the links repo into /app if not already a git repo.
  # This makes /app the redirective-links repo so `git pull` (webhook reload) works.
  if [ ! -d /app/.git ]; then
    echo "Initializing git repo for links..."
    cd /app
    git init -q
    git remote add origin "$LINKS_REPO"
    if git fetch origin main; then
      # The bundled stub links.yaml is untracked and would block the checkout;
      # stash a copy so we can restore it if checkout unexpectedly fails in dev.
      cp links.yaml /tmp/links.stub.yaml 2>/dev/null || true
      rm -f links.yaml
      if git checkout -b main origin/main; then
        echo "Links repository initialized from $LINKS_REPO."
      elif [ "$LINKS_REQUIRED" = "1" ]; then
        fail "'git checkout origin/main' failed"
      else
        echo "WARN: checkout failed; restoring bundled stub links.yaml (LINKS_REQUIRED!=1)." >&2
        cp /tmp/links.stub.yaml links.yaml 2>/dev/null || true
      fi
    elif [ "$LINKS_REQUIRED" = "1" ]; then
      fail "'git fetch origin main' failed"
    else
      echo "WARN: git fetch failed; keeping bundled stub links.yaml (LINKS_REQUIRED!=1)." >&2
    fi
  fi
elif [ "$LINKS_REQUIRED" = "1" ]; then
  fail "deploy key $DEPLOY_KEY is missing"
fi

# Belt-and-suspenders: in prod the links repo must actually be present. If we
# somehow reach here under LINKS_REQUIRED=1 without /app/.git, we would serve
# the stub — refuse.
if [ "$LINKS_REQUIRED" = "1" ]; then
  if [ ! -d /app/.git ]; then
    fail "/app/.git absent after init — links repo not present, would serve stub"
  fi
fi

# Execute the redirective binary
exec /usr/local/bin/redirective "$@"
