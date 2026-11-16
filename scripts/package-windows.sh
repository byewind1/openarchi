#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
python3 scripts/build-release.py --target x86_64-pc-windows-msvc
python3 scripts/package.py --target x86_64-pc-windows-msvc
