#!/usr/bin/env bash
# Native Ubuntu 22.04 build for standard GitHub-hosted runners.
# Keep the same libc baseline as the portable Docker builder without storing
# another OS image or a second copy of the source tree on the hosted VM.
set -euo pipefail
if [ "${GITHUB_ACTIONS:-}" != true ] || [ "${RUNNER_ENVIRONMENT:-}" != github-hosted ]; then
  echo 'This entry point is only for GitHub-hosted Actions runners' >&2
  exit 1
fi
. /etc/os-release
if [ "$ID" != ubuntu ] || [ "$VERSION_ID" != 22.04 ]; then
  echo 'Ubuntu 22.04 is required for the release ABI baseline' >&2
  exit 1
fi
check_disk() {
  df -h .
  local free_kb
  free_kb=$(df -Pk . | awk 'NR==2 {print $4}')
  if [ "$free_kb" -lt 15728640 ]; then
    echo 'Hosted runner has less than 15 GiB free; stopping without deleting data' >&2
    exit 1
  fi
}
check_disk
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0
cargo build --release --locked
ldd --version | sed -n '1p'
for bin in gravity lapce-proxy; do
  "target/release/$bin" --version
  mkdir -p target/ubuntu-22.04/release
  cp "target/release/$bin" "target/ubuntu-22.04/release/$bin"
done
df -h .
