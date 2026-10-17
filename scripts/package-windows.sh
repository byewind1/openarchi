#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version-display = "\(.*\)"/\1/p' Cargo.toml)
TARGET=x86_64-pc-windows-msvc
STAGE="dist/OpenArchi-$VERSION-Windows-x64"
ARCHIVE="dist/OpenArchi-$VERSION-Windows-x64.zip"

./scripts/cargo.sh xwin build --release --locked --target "$TARGET"

rm -rf "$STAGE"
mkdir -p "$STAGE"
cp "target/$TARGET/release/openarchi.exe" "$STAGE/OpenArchi.exe"
cp LICENSE README.md "$STAGE/"

cat > "$STAGE/README-Windows.txt" <<'EOF'
OpenArchi 0.1 for Windows x64

Run OpenArchi.exe. No installer or separate runtime is required.
This prototype is not code-signed; Windows SmartScreen may ask for confirmation.

Projects are saved as JSON through File > Save project.
EOF

rm -f "$ARCHIVE"
(cd dist && zip -qr "$(basename "$ARCHIVE")" "$(basename "$STAGE")")
printf '%s\n' "Packaged $ARCHIVE"
