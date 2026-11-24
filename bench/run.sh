#!/bin/sh
# Run from any directory. Python owns quoting, JSON and process monitoring.
set -eu
exec python3 "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/run.py" "$@"
