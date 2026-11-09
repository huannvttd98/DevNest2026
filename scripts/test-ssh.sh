#!/usr/bin/env bash
# Runs the Rust test-suite inside Docker, including SSH integration tests against a
# disposable OpenSSH server container.
#
#   bash scripts/test-ssh.sh            # from the repo root
#
# Requires Docker. Uses named volumes for the cargo registry / target dir so the
# Windows `target/` folder is never touched.
set -euo pipefail
export MSYS_NO_PATHCONV=1

ROOT_UNIX="$(cd "$(dirname "$0")/.." && pwd)"
# On Git Bash (Windows) Docker needs a Windows path.
ROOT="$( (cd "$ROOT_UNIX" && pwd -W) 2>/dev/null || echo "$ROOT_UNIX")"
NET=devnest-test-net
SSHD=devnest-test-sshd
IMAGE=devnest-rust-ci
PASSPHRASE="devnest-pass"

if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  docker build -t "$IMAGE" -f "$ROOT/scripts/ci.Dockerfile" "$ROOT/scripts"
fi

cleanup() { docker rm -f "$SSHD" >/dev/null 2>&1 || true; docker network rm "$NET" >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup
docker network create "$NET" >/dev/null

# Generate a passphrase-protected ed25519 key in a volume.
docker volume create devnest-test-keys >/dev/null
docker run --rm -v devnest-test-keys:/keys "$IMAGE" bash -c \
  "rm -f /keys/id_ed25519*; ssh-keygen -q -t ed25519 -N '$PASSPHRASE' -f /keys/id_ed25519"
PUBKEY="$(docker run --rm -v devnest-test-keys:/keys "$IMAGE" cat /keys/id_ed25519.pub)"

docker run -d --name "$SSHD" --network "$NET" \
  -e PUID=1000 -e PGID=1000 -e USER_NAME=dev -e USER_PASSWORD=devpass \
  -e PASSWORD_ACCESS=true -e PUBLIC_KEY="$PUBKEY" \
  lscr.io/linuxserver/openssh-server:latest >/dev/null

echo "waiting for sshd..."
for _ in $(seq 1 30); do
  if docker run --rm --network "$NET" "$IMAGE" bash -c "echo > /dev/tcp/$SSHD/2222" 2>/dev/null; then break; fi
  sleep 1
done

docker run --rm --network "$NET" \
  -v "$ROOT:/work" -v devnest-target:/target -v devnest-cargo:/usr/local/cargo/registry \
  -v devnest-test-keys:/keys \
  -e CARGO_TARGET_DIR=/target \
  -e DEVNEST_TEST_SSH_HOST="$SSHD" -e DEVNEST_TEST_SSH_PORT=2222 \
  -e DEVNEST_TEST_SSH_USER=dev -e DEVNEST_TEST_SSH_PASSWORD=devpass \
  -e DEVNEST_TEST_SSH_KEY=/keys/id_ed25519 -e DEVNEST_TEST_SSH_KEY_PASSPHRASE="$PASSPHRASE" \
  -w /work/src-tauri "$IMAGE" cargo test "$@"
