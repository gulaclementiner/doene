#!/usr/bin/env bash
# Launch Döne. Builds if needed, then runs the app.
#   ./run.sh              run (debug build)
#   ./run.sh --release    run an optimised build
# Data lives in the platform default dir; override with DONE_DATA_DIR, e.g.
#   DONE_DATA_DIR=./.data ./run.sh
set -euo pipefail
cd "$(dirname "$0")"

if [[ "${1:-}" == "--release" ]]; then
  shift
  exec cargo run --release -- "$@"
fi
exec cargo run -- "$@"
