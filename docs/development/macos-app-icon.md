# macOS app icon packaging and Dock verification

This repository has two icon paths that must stay in sync:

1. `icon.png` is the source image. Its alpha channel and transparent padding control the visible size and silhouette in Finder and Dock. Keep any designed interior background (the glossy black rounded square) opaque; make only the area outside that silhouette transparent. Leave enough transparent canvas around the symbol so its optical size matches nearby Dock icons.
2. `scripts/package-macos.sh` builds `Contents/Resources/OpenArchi.icns`; `packaging/Info.plist` must set `CFBundleIconFile` to `OpenArchi.icns`.
3. `src/main.rs` must also decode the same `icon.png` with `eframe::icon_data::from_png_bytes(include_bytes!("../icon.png"))` and pass it to `ViewportBuilder::with_icon`. Without this runtime setting, eframe supplies its own default icon to macOS: Finder can show the repository icon while the running Dock tile still shows eframe's black-and-white mark.

## ICNS chunk map

The packer uses PNG payloads with these tested ICNS type codes. Preserve the mapping in `scripts/png-to-icns.py`:

| ICNS type | Iconset PNG | PNG dimensions |
| --- | --- | ---: |
| `ic11` | `icon_16x16@2x.png` | 32 × 32 |
| `ic12` | `icon_32x32@2x.png` | 64 × 64 |
| `ic07` | `icon_128x128.png` | 128 × 128 |
| `ic13` | `icon_128x128@2x.png` | 256 × 256 |
| `ic08` | `icon_256x256.png` | 256 × 256 |
| `ic14` | `icon_256x256@2x.png` | 512 × 512 |
| `ic09` | `icon_512x512.png` | 512 × 512 |
| `ic10` | `icon_512x512@2x.png` | 1024 × 1024 |

Do not substitute a legacy `icp*` chunk code based only on its apparent pixel size: the payload representation matters. A bad type/payload pairing can still produce a file that Finder partially recognizes, while `iconutil` extracts corrupted small variants. The packer validates the PNG signature and exact dimensions before writing each chunk.

## Verify a change

1. Inspect `icon.png` on a checkerboard or other non-white background. Confirm real alpha outside the icon, no opaque white corners, a clean edge, and intentional transparent padding. Compare the visible artwork bounds rather than just canvas dimensions when adjusting optical size.
2. Run `./scripts/cargo.sh fmt --all -- --check` and `./scripts/cargo.sh check --locked --offline --all-targets`.
3. Run `./scripts/package-macos.sh`.
4. Check `plutil -lint dist/OpenArchi.app/Contents/Info.plist`, then round-trip the built resource with `iconutil -c iconset -o /private/tmp/openarchi.iconset dist/OpenArchi.app/Contents/Resources/OpenArchi.icns`. Inspect the extracted small and large variants, including alpha edges.
5. Check matching arm64 UUIDs with `dwarfdump --uuid target/release/openarchi` and `dwarfdump --uuid dist/OpenArchi.app/Contents/MacOS/openarchi`; run `codesign --verify --deep --strict dist/OpenArchi.app`.
6. Quit all running OpenArchi instances before rebuilding or judging Dock state. Launch exactly one fresh `dist/OpenArchi.app`, reveal the Dock, and inspect a current screenshot at normal Dock size next to neighboring apps. Do not use a Finder preview or an old running process as Dock evidence. If the image or package changes, repeat packaging and the Dock check.
7. Record the bundle path, UUID, signature result, screenshot-based Dock outcome and any remaining issue in `docs/validation.md`.

The current optical scale was tuned from Dock screenshots: in the 1254 × 1254 source canvas the edited icon occupies a 980 × 951 visible bounding box, with the exterior transparent. Treat that as a reference point, not a universal scale rule; compare against the actual neighboring icons after each design change.
