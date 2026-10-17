#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
./scripts/cargo.sh build --release --locked
OPENARCHI_BUNDLE="dist/OpenArchi.app"
OPENARCHI_CONTENTS="$OPENARCHI_BUNDLE/Contents"
OPENARCHI_ICONSET="$(mktemp -d)/OpenArchi.iconset"
trap 'rm -rf "$(dirname "$OPENARCHI_ICONSET")"' EXIT HUP INT TERM
mkdir -p "$OPENARCHI_CONTENTS/MacOS" "$OPENARCHI_CONTENTS/Resources" "$OPENARCHI_ICONSET"
rm -f "$OPENARCHI_BUNDLE/Contents/MacOS/atelier-bim"
cp target/release/openarchi "$OPENARCHI_BUNDLE/Contents/MacOS/openarchi"
cp packaging/Info.plist "$OPENARCHI_CONTENTS/Info.plist"

for size in 16 32 128 256 512; do
    sips -s format png -z "$size" "$size" icon.png --out "$OPENARCHI_ICONSET/icon_${size}x${size}.png" >/dev/null
    double_size=$((size * 2))
    sips -s format png -z "$double_size" "$double_size" icon.png --out "$OPENARCHI_ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
sips -s format png -z 1024 1024 icon.png --out "$OPENARCHI_ICONSET/icon_512x512@2x.png" >/dev/null
python3 scripts/png-to-icns.py "$OPENARCHI_ICONSET" "$OPENARCHI_CONTENTS/Resources/OpenArchi.icns"
# Local ad-hoc signing. This is not Apple notarization or a distribution identity.
codesign --force --sign - "$OPENARCHI_BUNDLE"
# Refresh the bundle timestamp so Launch Services and Dock notice replaced icon resources.
touch "$OPENARCHI_BUNDLE"
VERSION=$(sed -n 's/^version-display = "\(.*\)"/\1/p' Cargo.toml)
OPENARCHI_ARCHIVE="dist/OpenArchi-$VERSION-macOS-arm64.zip"
rm -f "$OPENARCHI_ARCHIVE"
ditto -c -k --sequesterRsrc --keepParent "$OPENARCHI_BUNDLE" "$OPENARCHI_ARCHIVE"
printf '%s\n' "Built $OPENARCHI_BUNDLE"
printf '%s\n' "Packaged $OPENARCHI_ARCHIVE"
