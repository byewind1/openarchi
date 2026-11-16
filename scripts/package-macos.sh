#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
python3 scripts/build-release.py "$@"
python3 scripts/package.py "$@"
