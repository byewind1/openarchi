# Native click-lock verification — 2026-10-29

Bundle UUID `4095BCED-962A-36A1-8AEC-BD27D78FB11F`; fresh instance PID 45765, isolated HOME `/tmp/openarchi-click-lock-20261029`. App path: repository `dist/OpenArchi.app`. Window 1512×988 logical points at (204,1), capture 3024×1976 pixels. Input was real CoreGraphics mouse move/down/up after activating the owned process, per the repository OpenArchi desktop fallback. AX identifies the active status and element count. These are desktop-tool observations, not REA Evidence IDs.

Both wall and polyline: enter edge Insert node → move over Curve → return to canvas (still Insert node) → left-click Curve → move over Rotate → return to canvas (still Curve). Only previews changed; counts stayed 12 for the wall and 13 for the disposable polyline project. Escape cancelled the preview. The user instance PID 85849 remained at 12 elements / 1 selected before and after.

## wall: return

```text
68 AXStaticText ||Insert node|
69 AXStaticText ||X 6.000 m   Y 8.000 m   |   12 elements|
202 AXStaticText ||12 elements|
```

## wall: click

```text
68 AXStaticText ||Curve edge|
69 AXStaticText ||X 6.000 m   Y 8.000 m   |   12 elements|
202 AXStaticText ||12 elements|
```

## wall: locked

```text
68 AXStaticText ||Curve edge|
69 AXStaticText ||X 6.000 m   Y 8.000 m   |   12 elements|
202 AXStaticText ||12 elements|
```

## poly: return

```text
44 AXStaticText ||Insert node|
45 AXStaticText ||X -3.500 m   Y 7.113 m   |   13 elements|
156 AXStaticText ||13 elements|
```

## poly: click

```text
44 AXStaticText ||Curve edge|
45 AXStaticText ||X -3.500 m   Y 7.113 m   |   13 elements|
156 AXStaticText ||13 elements|
```

## poly: locked

```text
44 AXStaticText ||Curve edge|
45 AXStaticText ||X -3.500 m   Y 7.113 m   |   13 elements|
156 AXStaticText ||13 elements|
```

Native coverage: English plan for wall and polyline. Both languages and wall projected-3D are covered by RawInput regressions; a new Chinese/3D desktop sequence and exact cursor hotspot/timing were not measured for this no-label-change fix.

Snapshots: [wall locked to Curve](wall-command-locked.png), [polyline locked to Curve](polyline-command-locked.png).
