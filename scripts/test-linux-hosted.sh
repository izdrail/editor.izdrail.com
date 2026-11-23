#!/usr/bin/env bash
# Acceptance gates on the existing standard GitHub-hosted Ubuntu runner.
set -euo pipefail
if [ "${GITHUB_ACTIONS:-}" != true ] || [ "${RUNNER_ENVIRONMENT:-}" != github-hosted ]; then
  echo 'Acceptance requires a GitHub-hosted Actions runner' >&2
  exit 1
fi
free_kb=$(df -Pk . | awk 'NR==2 {print $4}')
if [ "$free_kb" -lt 15728640 ]; then
  echo 'Less than 15 GiB free; stopping without deleting data' >&2
  exit 1
fi
ulimit -n 65536
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
mkdir -p target/acceptance-logs
python3 scripts/test-linux-build.py 2>&1 | tee target/acceptance-logs/packaging.log
cargo test --locked -p lapce-app smart_pairing_settings_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/pairing-settings.log
cargo test --locked -p lapce-app definition_identity_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/definition-identity.log
cargo test --locked -p lapce-app file_refs::tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/file-refs.log
cargo test --locked -p lapce-core smart_editing_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/smart-editing.log
cargo test --locked -p lapce-app editor_review_binding -- --test-threads=1 2>&1 | tee target/acceptance-logs/lexer.log
cargo test --locked -p lapce-app quiet_conversation_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/focused.log
cargo test --locked --workspace -- --test-threads=2 2>&1 | tee target/acceptance-logs/workspace.log
df -h .
