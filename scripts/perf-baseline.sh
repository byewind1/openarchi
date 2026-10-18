#!/bin/sh
# Local comparison baseline only; this is not part of CI.
set -eu
cd "$(dirname "$0")/.."
exec ./scripts/cargo.sh test --release --test performance_baseline -- --ignored --nocapture
