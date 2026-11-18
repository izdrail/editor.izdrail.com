#!/usr/bin/env bash
# Fixed ABI; source enters through git archive, never a host checkout mount.
set -euo pipefail
name="editor-abi-${GITHUB_RUN_ID:-local}"
container=''
watchdog=''
builder_pid=''
cleanup() {
  if [ -n "$watchdog" ]; then kill "$watchdog" 2>/dev/null || true; wait "$watchdog" 2>/dev/null || true; fi
  if [ -n "$builder_pid" ]; then kill -TERM "$builder_pid" 2>/dev/null || true; fi
  if [ -n "$container" ]; then docker rm -f "$container"; fi
}
trap cleanup EXIT INT TERM
# Shared named caches are single-writer even if more runners are later added
# to this machine. Distinct per-SHA workflow groups preserve every queued run.
exec 9>/tmp/gravity-editor-portable-build.lock
flock 9
free_kb=$(df -Pk / | awk 'NR==2 {print $4}')
if [ "$free_kb" -lt 26214400 ]; then
  echo "Below approved 25 GiB disk floor; no build started" >&2
  exit 1
fi
start=$SECONDS
# Cache the toolchain/system packages as an image keyed by its exact recipe.
# No Docker build context: only the Dockerfile is sent to the daemon.
recipe=scripts/linux-builder.Dockerfile
key=$(sha256sum "$recipe" | cut -c1-16)
image="gravity-editor-builder:${key}"
if docker image inspect "$image" > /tmp/gravity-builder-inspect.log 2>&1; then
  echo "Builder image cache hit: $image"
else
  cat /tmp/gravity-builder-inspect.log
  docker build --tag "$image" - < "$recipe" &
  builder_pid=$!
  (
    while sleep 5; do
      free_kb=$(df -Pk / | awk 'NR==2 {print $4}')
      if [ "$free_kb" -lt 26214400 ]; then
        echo "Disk floor crossed: stopping this builder process" >&2
        kill -TERM "$builder_pid"
        exit 1
      fi
    done
  ) &
  watchdog=$!
  wait "$builder_pid"
  kill "$watchdog"
  wait "$watchdog" 2>/dev/null || true
  watchdog=''
  builder_pid=''
fi
setup_end=$SECONDS
# Named volumes cache downloaded crates and compiled artifacts across ephemeral
# containers. They contain no source checkout or credentials and are not pruned.
container=$(docker create --label "instinct.editor.build=${GITHUB_RUN_ID:-local}" --name "$name" \
  --mount type=volume,src=gravity-editor-cargo-registry,dst=/usr/local/cargo/registry \
  --mount type=volume,src=gravity-editor-cargo-git,dst=/usr/local/cargo/git \
  --mount type=volume,src=gravity-editor-target-1980,dst=/src/target \
  --workdir /src "$image" sleep infinity)
docker start "$container"
# Preserve the approved 25 GiB floor. Never delete caches or unrelated data.
(
  while sleep 5; do
    free_kb=$(df -Pk / | awk 'NR==2 {print $4}')
    if [ "$free_kb" -lt 26214400 ]; then
      echo "Disk floor crossed: stopping this build container" >&2
      docker stop -t 5 "$container"
      exit 1
    fi
  done
) &
watchdog=$!
git archive HEAD | docker cp - "$container:/src"
docker exec "$container" bash -euo pipefail -c '
  python3 scripts/restore-source-mtimes.py
  cargo build --release --locked
  ldd --version | sed -n "1p"
  target/release/gravity --version
  target/release/lapce-proxy --version
'
mkdir -p target/ubuntu-22.04/release
for bin in gravity lapce-proxy; do
  docker cp "$container:/src/target/release/$bin" "target/ubuntu-22.04/release/$bin"
done
printf 'Timing: builder setup=%ss; compile/verify/copy=%ss; total=%ss\n' "$((setup_end-start))" "$((SECONDS-setup_end))" "$((SECONDS-start))"
