#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
./scripts/cargo.sh build --release --locked
OPENARCHI_BUNDLE="dist/OpenArchi.app"
mkdir -p "$OPENARCHI_BUNDLE/Contents/MacOS"
rm -f "$OPENARCHI_BUNDLE/Contents/MacOS/atelier-bim"
cp target/release/openarchi "$OPENARCHI_BUNDLE/Contents/MacOS/openarchi"
cp packaging/Info.plist "$OPENARCHI_BUNDLE/Contents/Info.plist"
# Local ad-hoc signing. This is not Apple notarization or a distribution identity.
codesign --force --sign - "$OPENARCHI_BUNDLE"
printf '%s\n' "Built $OPENARCHI_BUNDLE"
