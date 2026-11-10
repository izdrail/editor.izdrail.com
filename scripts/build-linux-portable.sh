#!/usr/bin/env bash
# Build in a fixed ABI without host-path mounts. Works when the runner itself
# is a Docker container talking to the host daemon through docker.sock.
set -euo pipefail
name="editor-abi-${GITHUB_RUN_ID:-local}-$$"
container=''
cleanup() { if [ -n "$container" ]; then docker rm -f "$container" >/dev/null; fi; }
trap cleanup EXIT INT TERM
container=$(docker create --name "$name" --workdir /src ubuntu:22.04 sleep infinity)
docker start "$container" >/dev/null
# git archive excludes local caches, credentials and .git.
git archive HEAD | docker cp - "$container:/src"
docker exec "$container" bash -euo pipefail -c '
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential git curl ca-certificates xz-utils clang pkg-config libxkbcommon-x11-dev libvulkan-dev libwayland-dev xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev cmake
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98.0 --profile minimal
  export PATH=/root/.cargo/bin:$PATH
  cargo build --release --locked
  ldd --version | head -n 1
  target/release/lapce --version
  target/release/lapce-proxy --version
'
mkdir -p target/ubuntu-22.04/release
for bin in lapce lapce-proxy; do
  docker cp "$container:/src/target/release/$bin" "target/ubuntu-22.04/release/$bin"
done
