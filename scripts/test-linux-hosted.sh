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
python3 bench/test_bench.py
python3 scripts/test-laravel-lint.py
ulimit -n 65536
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
mkdir -p target/acceptance-logs
python3 scripts/test-linux-build.py 2>&1 | tee target/acceptance-logs/packaging.log
cargo test --locked -p lapce-app --features logos-php form_input_traversal_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/form-input-traversal.log
cargo test --locked -p lapce-app lexer_setting_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/lexer-setting.log
cargo test --locked -p lapce-app settings_dropdown_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/settings-dropdown.log
cargo test --locked -p lapce-proxy search_preview_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/search-preview.log
cargo test --locked -p lapce-app search_status_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/search-status.log
cargo test --locked -p lapce-app settings_search_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/settings-search.log
cargo test --locked -p lapce-app smart_pairing_settings_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/pairing-settings.log
cargo test --locked -p lapce-app match_pairs_modifier_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/match-pairs-modifiers.log
cargo test --locked -p lapce-app definition_identity_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/definition-identity.log
cargo test --locked -p lapce-app file_refs::tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/file-refs.log
cargo test --locked -p lapce-core pairing_context -- --test-threads=1 2>&1 | tee target/acceptance-logs/pairing-context.log
cargo test --locked -p lapce-core smart_editing_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/smart-editing.log
cargo test --locked -p lapce-app editor_review_binding -- --test-threads=1 2>&1 | tee target/acceptance-logs/lexer.log
cargo test --locked -p lapce-app quiet_conversation_tests -- --test-threads=1 2>&1 | tee target/acceptance-logs/focused.log
LAPCE_SNIPPET_EXPORT="$PWD/target/acceptance-logs/laravel-expansions.json" cargo test --locked -p lapce-app export_laravel_expansions_for_php_lint -- --ignored --exact builtin_snippets::tests::export_laravel_expansions_for_php_lint 2>&1 | tee target/acceptance-logs/snippet-export.log
python3 scripts/lint-laravel-expansions.py target/acceptance-logs/laravel-expansions.json 2>&1 | tee target/acceptance-logs/php-snippets.log
LAPCE_BLADE_EXPORT="$PWD/target/acceptance-logs/blade-expansions.json" cargo test --locked -p lapce-app export_blade_expansions_for_compile -- --ignored --exact builtin_snippets::tests::export_blade_expansions_for_compile 2>&1 | tee target/acceptance-logs/blade-export.log
php scripts/compile-blade-fixtures.php target/acceptance-logs/blade-expansions.json 2>&1 | tee target/acceptance-logs/blade-compile.log
php scripts/runtime-laravel-fixtures.php target/acceptance-logs/laravel-expansions.json 2>&1 | tee target/acceptance-logs/laravel-runtime.log
cargo test --locked -p lapce-php 2>&1 | tee target/acceptance-logs/php-context.log
cargo test --locked -p gravity-lex --test corpus -- --nocapture 2>&1 | tee target/acceptance-logs/logos-php-token-oracle.log
cargo test --locked --manifest-path lexilla-rs/Cargo.toml 2>&1 | tee target/acceptance-logs/logos-style-import.log
cargo test --locked -p lapce-core --features logos-php logos_import -- --test-threads=1 2>&1 | tee target/acceptance-logs/logos-import.log
cargo run --locked -p lapce-core --features logos-php --example snapshot_perf -- --compare --quick > target/acceptance-logs/logos-benchmark-smoke.json
python3 scripts/check-logos-benchmark.py target/acceptance-logs/logos-benchmark-smoke.json
for backend in lexilla logos; do
  GRAVITY_LEXER=$backend SNAPSHOT_PERF_SYNTAX=$backend cargo run --locked -p lapce-core --features logos-php --example snapshot_perf -- --compare --quick > "target/acceptance-logs/logos-syntax-$backend-smoke.json"
done
cargo test --locked --workspace -- --test-threads=2 2>&1 | tee target/acceptance-logs/workspace.log
cargo test --locked -p lapce-proxy native_50k_proxy_baseline -- --ignored --nocapture 2>&1 | tee target/acceptance-logs/php-native-proxy-benchmark.log
cargo test --locked -p lapce-app native_50k_adapter_baseline -- --ignored --nocapture 2>&1 | tee target/acceptance-logs/php-native-adapter-benchmark.log
df -h .
