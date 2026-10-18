# Bundled CJK font

`NotoSansSC-Regular.ttf` is Noto Sans SC Regular, bundled so that the
Simplified Chinese interface renders consistently on macOS, Windows and Linux
even when no suitable system CJK font is installed.

- Source: Noto Sans SC Regular (`NotoSansCHI-Regular.ttf` in the local macOS
  font collection); upstream project: https://github.com/notofonts/noto-cjk
- Copyright: (c) 2014-2021 Adobe, with Reserved Font Name "Source"
- License: SIL Open Font License 1.1, reproduced in `OFL.txt`
- SHA-256: `ae82f4e2a55e1316a55bcc1d05e9555ce08d8bda07e893b486896b626fd852ff`

The application prefers an installed system CJK font and falls back to this
bundled copy. Microsoft YaHei, SimHei and SimSun are never redistributed with
OpenArchi; only paths to locally installed Windows fonts are probed.
