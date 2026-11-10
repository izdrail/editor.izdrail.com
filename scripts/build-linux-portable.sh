#!/usr/bin/env bash
# Build in a fixed ABI without host-path mounts. Works when the runner itself
# is a Docker container talking to the host daemon through docker.sock.
set -euo pipefail
name="editor-abi-${GITHUB_RUN_ID:-local}"
container=''
watchdog=''
cleanup() { if [ -n "$watchdog" ]; then kill "$watchdog" 2>/dev/null || true; fi; if [ -n "$container" ]; then docker rm -f "$container" >/dev/null; fi; }
trap cleanup EXIT INT TERM
container=$(docker create --label "instinct.editor.build=${GITHUB_RUN_ID:-local}" --name "$name" --workdir /src ubuntu:22.04 sleep infinity)
docker start "$container" >/dev/null
# The runner filesystem is on dell root. Stop only this build if the
# approved 30 GiB floor is crossed, even during a long compiler step.
(
  while sleep 5; do
    free_kb=$(df -Pk / | awk 'NR==2 {print $4}')
    if [ "$free_kb" -lt 31457280 ]; then
      echo "Disk floor crossed: stopping this build container" >&2
      docker stop -t 5 "$container" >/dev/null
      exit 1
    fi
  done
) &
watchdog=$!
# git archive excludes local caches, credentials and .git.
git archive HEAD | docker cp - "$container:/src"
docker exec "$container" bash -euo pipefail -c '
  apt-get update -qq
  DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential git curl ca-certificates xz-utils clang pkg-config libxkbcommon-x11-dev libvulkan-dev libwayland-dev xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev libssl-dev cmake
  curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.98.0 --profile minimal
  export PATH=/root/.cargo/bin:$PATH
  cargo build --release --locked
  ldd --version | sed -n "1p"
  target/release/lapce --version
  target/release/lapce-proxy --version
'
mkdir -p target/ubuntu-22.04/release
for bin in lapce lapce-proxy; do
  docker cp "$container:/src/target/release/$bin" "target/ubuntu-22.04/release/$bin"
done
