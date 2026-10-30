# Endpoint icon palette — native evidence, 2026-10-30

Final bundle UUID: `8A99BFD7-6D77-3566-B8E0-5F2404D5D802`. Packaged app `dist/OpenArchi.app`, disposable PID 71029, isolated HOME `/tmp/openarchi-endpoint-palette-20261030`. Window 1512×988 points at (204,1); PNG 1512×988 pixels (1×). This session differs in display scale from the earlier 2× captures.

Steps: select demo Wall 004, click its start reference-line endpoint, inspect the ten-icon palette. Click Curve edge, hover Rotate, return to canvas: Curve stays active. Click first slot: returns to the existing endpoint Move node state without changing the 12-element model. View → Language English → Chinese → English preserves visible selection/count; final screenshots show the same endpoint menu in both languages. A second instance (PID 75427, isolated HOME confirmed through process environment) restarted with stored zh-CN: first AX title 未命名. Both owned instances closed normally; test preference restored to en. Existing user PID 60862 left running; no access to its project, no unsaved state changed.

Input/capture: repository-authorized CoreGraphics fallback for OpenArchi egui; fresh AX trees identify commands. These are desktop-tool captures, not REA Evidence IDs.

## oa-endpoint-menu-en.txt

```text
1 AXWindow |Untitled||
68 AXStaticText ||Wall reference-line endpoint edit · drag the handle to reshape the wall|
69 AXStaticText ||X 0.000 m   Y 8.000 m   |   12 elements|
209 AXButton |Move node||
210 AXButton |Curve edge||
215 AXButton |Rotate||
```

## oa-endpoint-locked.txt

```text
1 AXWindow |Untitled||
68 AXStaticText ||Curve edge|
69 AXStaticText ||X 1.475 m   Y 7.262 m   |   12 elements|
204 AXButton |Move node||
205 AXButton |Curve edge||
210 AXButton |Rotate||
```

## oa-endpoint-return-settled.txt

```text
1 AXWindow |Untitled||
68 AXStaticText ||Wall reference-line endpoint edit · drag the handle to reshape the wall|
69 AXStaticText ||X 0.775 m   Y 7.262 m   |   12 elements|
209 AXButton |Move node||
210 AXButton |Curve edge||
215 AXButton |Rotate||
```

## oa-endpoint-menu-zh.txt

```text
1 AXWindow |未命名||
69 AXStaticText ||X 0.775 m   Y 7.338 m   |   12 个构件|
209 AXButton |移动节点||
210 AXButton |曲边||
215 AXButton |旋转||
```

## oa-endpoint-restart-zh.txt

```text
1 AXWindow |未命名||
```

Screenshots: [English](endpoint-en.png), [Chinese](endpoint-zh.png).

Native scope: plan start-endpoint menu and shared-command roundtrip. Both endpoints, direct movement/snap, cancellation and undo/redo have automated regression coverage. The end endpoint and every icon operation were not all replayed natively; 3D isolated-endpoint entry was not extended. Exact cursor hotspots/timing and new AC29 equivalence remain unknown.
