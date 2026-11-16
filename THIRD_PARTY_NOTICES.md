# Third-party notices

OpenArchi source is licensed under Apache-2.0. Dependencies retain their own licenses.

- Noto Sans SC: SIL Open Font License 1.1; the license is in `assets/fonts/OFL.txt`. The egui default font bundle also embeds Hack, Noto Emoji, Ubuntu and an emoji icon font; their MIT, OFL and Ubuntu font licenses are included under Licenses/dependencies/epaint_default_fonts-0.36.2.
- AccessKit macOS (including the local teardown fix): MIT OR Apache-2.0; both licenses are in `vendor/accesskit_macos/`.
- Rust dependencies, including egui/eframe, wgpu, serde, geo and image, are pinned in Cargo.lock. Distribution includes the dependency license texts in Licenses/dependencies.
- The optional MCP adapter uses the external Python MCP SDK environment declared in tools/mcp-adapter/pyproject.toml and uv.lock. That environment is not bundled.
- DWG conversion programs, vendor CAD libraries and system fonts are not bundled. Users obtain and license any external converter themselves.

The icon, synthetic sample constructors and application code are original OpenArchi material. No branded CAD application assets are distributed.
