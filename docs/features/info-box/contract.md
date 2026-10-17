# Info Box wall properties — Archicad 29 / OpenArchi

## Goal and bounded scope

Recreate `Window > Palettes > Info Box` for a single selected wall. When the
palette is on, the horizontal Info Box shows the wall properties most often
needed from `⌘T` **Wall Settings**, lets the user edit them in place, and writes
the result through the same validated, undoable model operation used by the
settings dialog. Plan and 3D therefore read the same updated wall.

Target: Archicad 29 `29.0.0 v1 INT (5101)`, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`.
OpenArchi uses meters, three displayed decimals, a single wall selection and the
session's active home story. With the Wall tool active and nothing selected the
same palette shows the next wall's creation defaults. Multi-wall common-value
editing and Archicad's floating-palette docking behavior are not implemented in
this slice.

## Evidence

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| `Window > Palettes > Info Box` exists and controls an Info Box palette | runtime observation | CUA accessibility-tree observation of the running Archicad 29, 2026-10-18 | CUA observation only; no REA Evidence ID. The user's project was not saved or modified. |
| With one wall selected, the Info Box exposes `All Selected: 1`, `Layer`, `Structural - Bearing`, `Geometry Method`, `Reference Line Location`, `Structure: Generic Wall/Shell`, `Floor Plan and Section…`, `Linked Stories: 1. Story (Home + 1) / 0. Ground Floor (Current)`, and `Bottom and Top: 3000 / 0` | runtime observation | Same CUA accessibility-tree observation | Field visibility is for that selection and workspace state. Exact control widths, hidden flyouts, and all dropdown contents were not enumerated. |
| The corresponding `⌘T` groups and defaults are already recorded | runtime observation | [Wall settings](../wall-settings.md), same target/build | Values belong to the AC29 `Untitled` template and may differ in another project. |
| Editing a wall setting in the Info Box applies to the model rather than only changing display text | product requirement and inferred interaction model | User request; `⌘T` runtime evidence plus OpenArchi's existing `WallSettingsDraft` contract | AC29's exact per-keystroke versus focus-loss commit timing was not measured. |
| With the Wall tool active and nothing selected, the AC29 Info Box shows `Default Settings`, Layer `Structural - Bearing`, four Geometry Method buttons, Reference Line Location `Core Outside` with an offset popup, Structure `Generic Wall/Shell`, `Floor Plan and Section…`, Linked Stories `1. Story (Home + 1)`, Home Story `0. Ground Floor (Current)`, and Bottom and Top `3000 / 0` | runtime observation | CUA screenshots `/tmp/ac29-wall-tool.png`, `/tmp/ac29-info-left.png`, `/tmp/ac29-info-right.png`, Archicad 29 `29.0.0 v1 INT (5101)`, 2026-10-18 | CUA observation only; no REA Evidence ID. The AC29 project was not modified. Control widths and every flyout were not enumerated. |
| The Structure control opens a composite selector with `Exterior` / `Interior` groups and a component list | runtime observation | Same CUA capture; the flyout was opened and dismissed with Escape | Component names and editing behavior were not exercised in AC29. |

The earlier selected-wall AC29 pass returned `Screenshot unavailable` for native
capture. The wall-tool pass produced the CUA screenshots listed above, but the
comparison remains field- and layout-level rather than pixel-level. OpenArchi's
layout is an original egui interpretation, not a pixel copy.

## Behavior contract

### Entry and visibility

- `Window > Palettes > Info Box` toggles the Info Box for the current session.
  The menu item uses the same selected state as the palette. `Reset workspace
  layout` turns the Info Box back on.
- With exactly one wall selected and the Info Box on, the bar shows the wall ID,
  `All Selected: 1`, Layer, Structural function, Geometry Method, Reference
  Line, Reference Offset, Thickness, Height, Top Offset, Bottom Offset, Top
  Link, Structure/profile name, `Floor Plan and Section…`, `Settings…`, and
  `Apply`.
- With no wall selected and a non-Wall tool active, a multiple selection, or a
  non-wall element, the wall Info Box fields are absent. With the Wall tool
  active and no element selected, the `Default Settings` state below applies.
  This slice does not merge common values across a multi-wall selection.

### Wall-tool default state (no selection)

- With the Wall tool active and no element selected, the Info Box switches from
  the selected-wall form to `Default Settings`. It shows the next wall's Layer,
  Geometry Method, Reference Line, Reference Offset, Structure, Thickness,
  Height, Top Link, Top Offset and Bottom Offset, followed by `Floor Plan and
  Section…`, `Settings…` and `Apply`.
- Edits use the same `WallSettingsDraft` validation as the settings dialog. A
  valid change updates the Wall tool defaults held in session UI state; it does
  not create an undo entry or mark the project dirty. An invalid value stays
  visible with a localized error and leaves the previous defaults unchanged.
- Geometry Method switches the actual Wall tool input state. Single uses two
  endpoints; Continuous accepts successive nodes and finishes by double-click
  or Enter; Rectangular uses two diagonal corners and emits four world-aligned
  sides; Polygonal accepts contour nodes and closes on the first node, Enter or
  double-click. Changing the method cancels an unfinished draft. Details and
  exclusions are in [wall geometry methods](../wall-geometry-methods.md).
- Clicking the plan to start a wall first commits a pending valid default draft.
  The created wall copies layer, geometry method, reference line and offset,
  structure name, thickness, height, top link and top/bottom offsets.
- `⌘T`, `Settings…` and `Floor Plan and Section…` open **Wall Default Settings**
  with the current defaults; the last entry expands the floor-plan group.
- Material, Snap and Ortho shown below the tool defaults are OpenArchi
  extensions, not fields observed in the AC29 wall-tool Info Box.

### Editing and commit semantics

- `DragValue` edits mark the draft dirty and commit when the drag stops or the
  field loses focus. Dropdown and segmented-button changes commit immediately.
- `Apply` commits a dirty draft. `Settings…` and `⌘T` first commit a pending
  Info Box edit, then open `⌘T` **Wall Settings**. `Floor Plan and Section…`
  does the same and opens the dialog with that group expanded.
- Selection changes and hiding the palette commit a pending draft before the
  old wall disappears from the Info Box. If validation fails, the old wall and
  its error stay visible instead of silently discarding the edit.
- One successful commit is one `Editor::replace_element` call and therefore one
  undo entry. `Undo` restores the wall in Plan and 3D; OpenArchi's existing
  undo behavior clears the selection, so the Info Box closes until the wall is
  selected again.

### Model effects and validation

| Field | Effect |
|---|---|
| Thickness, Height | Update the wall shape used by Plan and 3D. |
| Reference Line (`Center`/`Inside`/`Outside`), Reference Offset | Move the wall body relative to the fixed blue reference line; the line itself stays on `start/end`. |
| Layer | Updates layer visibility and filtering; the wall remains the same element. |
| Structural function | Persists the `WallStructuralSettings` value. It is metadata in this slice and does not create an analytical model. |
| Geometry Method | For the Wall tool, selects the creation state machine. For an existing wall it is stored metadata and does not convert the wall. |
| Top Link, Structure/profile name, Top/Bottom Offset | Persist in the wall settings; thickness/height and top/bottom offsets update the current solid where supported. |

Validation is shared with `⌘T`: thickness must be 1 mm–5 m, height 10 mm–100 m,
names at most 128 characters, and coordinates finite with a minimum length.
Errors are shown in the Info Box in the active UI language; the project and undo
history remain unchanged.

### Explicit exclusions

- Common-value editing for multiple selected walls and mixed selections.
- Archicad's composite-structure browser and component editor. OpenArchi's
  `Structure` control edits and persists the profile name; it does not list or
  generate composite components.
- Archicad-specific palette docking, saved workspace files, and cross-session
  palette visibility.
- Analytical-model generation or rendering changes for Structural function.
- Pixel-level visual equivalence to Archicad controls, icons, or flyouts.

## Acceptance checklist

| Scenario | Expected result | Status |
|---|---|---|
| Toggle `Window > Palettes > Info Box` twice | Palette hides and returns; project and undo history unchanged | implemented; unit-tested. The final bundle's submenu opened with a checked `Info Box` item, but desktop automation could not activate the item, so the menu click path is not desktop-verified |
| Activate the Wall tool with nothing selected | Info Box shows `Default Settings` and all creation-default fields; selected-wall fields such as `All Selected` are absent | pass — desktop in Chinese and English; English pass read `Wall`, `Default Settings`, `Layer:`, `Geometry Method:`, `Reference Line:`, `Reference Offset`, `Structure:`, `Thickness`, `Height`, `Top Link:`, `Top Offset`, `Bottom Offset`, `Floor Plan and Section…`, `Settings…`, `Apply` on the final-content test copy |
| Change Geometry Method and draw | Each choice changes the click sequence, preview, commit count and undo scope; the result is not four labels over one rectangle workflow | implemented; workflow and RawInput regressions cover all four methods, chain Backspace, double-click completion and one-step undo |
| Edit Thickness `0.220 → 0.360 m`, Height `3.000 → 4.200 m`, Reference Line `Outside`, offset `0.150 m` | Values persist in the tool state; status reports the defaults as applied; Undo stays disabled and the element count is unchanged | pass — desktop: values survived a Wall → Select → Wall tool switch; `撤销` stayed disabled at 12 elements |
| Draw a wall after editing the defaults | The new wall inherits layer, reference line and offset, thickness, height, top link, structure and offsets; selection Info Box shows the same values; the count increases and Undo becomes available | pass — desktop: created `墙 013` at 4.000 m / 90°, inherited `外部`, `0.150 m`, `0.360 m`, `4.200 m`, `Generic Wall/Shell`, layer `03 建筑`, and showed the selected-wall Info Box; count `12 → 13`, `撤销` enabled |
| Enter an invalid thickness in the tool state | Defaults and project stay unchanged; localized error remains visible | implemented; unit-tested (`0.001 m`) |
| `⌘T`, `Settings…`, `Floor Plan and Section…` in the tool state | Each opens **Wall Default Settings** with the current defaults; the floor-plan entry expands that group | pass — desktop: `设置…` opened `墙默认设置` with the edited values; `⌘T` and the floor-plan button covered by unit regressions |
| Select one wall with the palette on | Common wall fields and `All Selected: 1` appear in English and Simplified Chinese | pass — desktop in Chinese (`墙 009`, `全部已选: 1`, layer, structural function, geometry method, reference line, thickness `0.220 m`, height `3.000 m`, offsets, top link, `Generic Wall/Shell`, `平面图与剖面…`, `设置…`, `应用`); English field list covered by egui render test |
| Edit Thickness, Height, Reference Line and Reference Offset | Model changes immediately; Plan and 3D share the new wall; one Undo restores all values | pass for Height (`3.000 m → 3.500 m`, visible in 3D, one `⌘Z` restored it); pass for Reference Line `Inside`/`Outside` and offset `0.200 m` (blue line fixed, wall body moved); Thickness and the combined single-undo case covered by unit tests |
| Enter thickness `0.001 m` | No model or undo change; localized validation error remains visible | implemented; unit-tested |
| Change selection while a draft is pending | Pending edit is written to the old wall before the new wall loads | implemented; unit-tested |
| Press `⌘T` while a draft is pending | Pending edit is committed, then Wall Settings opens with the updated values | implemented; unit-tested |
| `Floor Plan and Section…` / `Settings…` | Opens Wall Settings; the former expands the floor-plan group | pass — desktop: both buttons opened Wall Settings with current values; after scrolling, the `Floor Plan and Section` group was visibly expanded |
| Multiple walls, non-wall, no selection, save/reopen | No merged wall Info Box; saved wall settings survive through the existing project format | multiple/no-selection fields deferred; persistence covered by wall-settings tests |
| Switch UI language with a wall selected | Info Box fields follow English/Simplified Chinese; project, selection and undo availability stay unchanged | pass — desktop: `简体中文 → English → 简体中文` with the wall still selected, `12 elements`, and unchanged `Undo`/`Redo`; restarting the bundled app showed Chinese on the first frame |

## Delivery evidence

Source:

- `src/app.rs`: `InfoBoxWallState`, `sync_info_box_wall`,
  `commit_info_box_wall`, `toggle_info_box_visibility`,
  `open_info_box_wall_settings`, `info_box_wall_properties`,
  `InfoBoxWallToolState`, `sync_info_box_wall_tool`,
  `commit_info_box_wall_tool`, `apply_wall_default_draft`,
  `info_box_wall_tool_properties`, `wall_preview_segments`,
  `finish_wall_draft`.
- `src/editor.rs`: `click_wall`, `finish_wall`, `backspace_wall`,
  `cancel_wall_draft`, `create_wall_segments`.
- `src/wall_ui.rs`: `WallSettingsDraft`, localized layer labels and top-link
  labels, `localized_error`.

Checks and packaged-app evidence are recorded in
[validation.md](../../validation.md) in the 2026-10-18 Info Box section.
