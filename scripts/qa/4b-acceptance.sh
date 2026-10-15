#!/usr/bin/env bash
# 4b reviewed-edits GUI acceptance driver (Xvfb + xdotool + scrot).
# NOT LOCALLY TESTED. Click targets are configured in a coords file because
# widget positions are only known from real screenshots. See docs/ai/reviewed-edits.md.
#
# Usage: scripts/qa/4b-acceptance.sh /path/to/lapce-binary [coords.env]
# coords.env defines X_/Y_ pairs: COMPOSER, PREPARE, APPLY, DISCARD, RESTORE (ints).
# Output: $OUT (default ./qa-4b-out) with PNGs per step and result.txt. Exit 1 on any failed assertion.
set -euo pipefail
BIN=${1:?lapce binary}
COORDS=${2:-"$(dirname "$0")/4b-coords.env"}
OUT=${OUT:-$PWD/qa-4b-out}
DISP=${DISPLAY_NUM:-:97}
W=${W:-1600}; H=${H:-1000}
mkdir -p "$OUT"; : >"$OUT/result.txt"
[ -f "$COORDS" ] || { echo "missing coords file $COORDS" >&2; exit 2; }
# shellcheck disable=SC1090
. "$COORDS"

PROJ=$(mktemp -d); HOME_DIR=$(mktemp -d)
trap 'kill ${APP_PID:-0} ${XVFB_PID:-0} 2>/dev/null || true; rm -rf "$PROJ" "$HOME_DIR"' EXIT
mkdir -p "$PROJ/app"
printf '<?php\nclass User {}\n' >"$PROJ/app/User.php"
printf '<?php\nclass Post {}\n' >"$PROJ/app/Post.php"
sha() { sha256sum "$PROJ/app/User.php" "$PROJ/app/Post.php" | awk '{print $1}' | tr '\n' ' '; }
BASE=$(sha)

pass() { echo "PASS $1" | tee -a "$OUT/result.txt"; }
fail() { echo "FAIL $1" | tee -a "$OUT/result.txt"; FAILED=1; }
FAILED=0
shot() { DISPLAY=$DISP scrot -o "$OUT/$1.png"; }
click() { DISPLAY=$DISP xdotool mousemove "$1" "$2" click 1; sleep 1; }
typeit() { DISPLAY=$DISP xdotool type --delay 5 -- "$1"; sleep 1; }
key() { DISPLAY=$DISP xdotool key "$@"; sleep 1; }
assert_src_unchanged() { [ "$(sha)" = "$BASE" ] && pass "$1: source files unchanged" || fail "$1: source files changed on disk"; }

Xvfb "$DISP" -screen 0 "${W}x${H}x24" & XVFB_PID=$!
sleep 2
export DISPLAY=$DISP HOME=$HOME_DIR XDG_CONFIG_HOME=$HOME_DIR/.config XDG_DATA_HOME=$HOME_DIR/.local/share
"$BIN" "$PROJ" "$PROJ/app/User.php" "$PROJ/app/Post.php" & APP_PID=$!
sleep 8; shot 00-launch
kill -0 $APP_PID 2>/dev/null && pass "launch: editor alive" || { fail "launch: editor exited"; exit 1; }

JSON='{"changes":[{"path":"app/User.php","replacement":"<?php\nclass User { public $a; }\n"},{"path":"app/Post.php","replacement":"<?php\nclass Post { public $b; }\n"}]}'
click "$COMPOSER_X" "$COMPOSER_Y"; typeit "$JSON"; shot 01-composer
click "$PREPARE_X" "$PREPARE_Y"; shot 02-review
assert_src_unchanged "prepare"
# Discard leaves everything unchanged
click "$DISCARD_X" "$DISCARD_Y"; shot 03-discarded
assert_src_unchanged "discard"
# Prepare again, apply
click "$PREPARE_X" "$PREPARE_Y"; click "$APPLY_X" "$APPLY_Y"; shot 04-applied
assert_src_unchanged "apply (buffers only, no save)"
# Restore reverts buffers
click "$RESTORE_X" "$RESTORE_Y"; shot 05-restored
assert_src_unchanged "restore"
# Per-document Ctrl+Z path is verified visually: compare 04/05 screenshots.
[ -d "$HOME_DIR" ] && find "$HOME_DIR" -path '*checkpoints*' -name '*.json' -printf '%m %p\n' >"$OUT/checkpoint-perms.txt" || true
if [ -s "$OUT/checkpoint-perms.txt" ]; then
  grep -qv '^600 ' "$OUT/checkpoint-perms.txt" && fail "checkpoint files not mode 0600" || pass "checkpoint files mode 0600"
else fail "no checkpoint record written"; fi
echo "Review PNGs in $OUT manually: two-file diff readable, Apply/Restore visible state changes." | tee -a "$OUT/result.txt"
exit $FAILED
