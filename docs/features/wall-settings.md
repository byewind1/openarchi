# Wall settings and properties — Archicad 29 / OpenArchi

## Goal

Reconstruct the Archicad 29 wall settings shown by `⌘T` as a reusable,
typed property editor in OpenArchi. A selected wall opens **Wall Settings**;
with no selection and the Wall tool active, `⌘T` opens **Wall Default
Settings** for the next wall. Every group observed in AC29 is represented,
editable, validated, persisted with the project, and synchronized where the
current native model supports it.

## Evidence

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| Wall settings are opened by `⌘T`; the window is titled `Wall Default Settings` when no wall is selected and the Wall tool is active | runtime observation | CUA capture of the running Archicad 29, 2026-10-18, target `29.0.0 v1 INT (5101)`, SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505` | CUA observation has no REA Evidence ID. The live `Untitled` project was not modified; the dialog was dismissed with Cancel. |
| Top-level groups are `Geometry and Positioning`, `Profile Offset Modifiers`, `Floor Plan and Section`, `Model`, `Structural Analytical Parameters`, `Classification and Properties` | runtime observation | Same CUA capture, expanded one group at a time | Collapsed-group internals below come from the same capture when expanded; no claim is made about groups not expanded in that pass. |
| Geometry defaults: Top Link `1. Story (Home + 1)`, height `3000`, top/bottom offsets `0`, Home Story `0. Ground Floor`, Project Zero `0`, Structure `Generic Wall/Shell`, reference offset `0` | runtime observation | Same CUA capture | Values belong to the current AC29 Untitled project and may differ in another template. |
| Floor-plan defaults: `All Relevant Stories`, `Projected`, `To Floor Plan Range`, `None`, `Solid`, `0.18 mm / 87`, `Both` | runtime observation | Same CUA capture | Display semantics are AC29 documentation/runtime; OpenArchi rendering coverage is stated below. |
| Model defaults: Junction Order `8`, surfaces `Stucco - Yellow Rough` / `Stucco - Yellow Rough` / `Stucco - White Rough`, relation `Zone Boundary` | runtime observation | Same CUA capture | Surface names are content values, not a material-system implementation. |
| Structural defaults: `Undefined`, Structural Member/Ignore small openings/Stretch & Cutback/Offset on, `ez 0`, model type `Shell`, all six supports `Rigid` | runtime observation | Same CUA capture | Analytical behavior in AC29 was not exercised. |
| Classification defaults: ID `SW - 014`, Option Status `(Main Model)`, Renovation Status `New`, renovation filter `All Relevant Filters`, most engineering fields `<Undefined>` | runtime observation | Same CUA capture | No classification-system file was edited or imported. |

No Archicad source, private resource or branded asset was copied. The OpenArchi
dialog uses original egui controls and original vector disclosure indicators.

## Behavior contract

### Entry and scope

- With exactly one wall selected, `⌘T` and `Properties > Edit Wall Settings`
  open **Wall Settings** for that wall.
- With the Wall tool active and no single selected wall, `⌘T` opens
  **Wall Default Settings**. Applying defaults changes the next wall's
  thickness, height, story, layer, renovation status and full `WallSettings`.
- With the Wall tool active and no selection, the Info Box `Default Settings`
  state edits the same defaults in place: layer, geometry method, reference
  line and offset, structure, thickness, height, top link and top/bottom
  offsets. A valid change updates session defaults without an undo entry, and
  the next drawn wall inherits the committed values.
- The geometry method is an input-mode setting, not merely a stored label.
  Single, Continuous, Rectangular and Polygonal each have their own click and
  finish state machine; see [wall geometry methods](wall-geometry-methods.md).
- With one non-wall selected, `⌘T` keeps the existing generic element settings.
- `Escape`, **Cancel**, or clicking outside closes without changing the model
  or the default settings.

### Editing and commit semantics

- **Apply** validates the draft, writes it, and keeps the dialog open.
- **OK** validates, writes once, and closes. One Apply/OK is one undo entry for
  a selected wall because it calls `Editor::replace_element` once.
- **Cancel/Escape** does not write. The dialog edits a clone, so partial edits
  never leak into the project.
- Default settings are session UI state: applying them does not create an undo
  entry or mark the project dirty. New walls copy the current defaults.
- `WallSettingsDraft::validate` rejects non-finite numbers, thickness outside
  1 mm–5 m, height outside 10 mm–100 m, names over 128 characters and walls
  shorter than 10 mm. `Project::validate` re-checks the persisted settings.
- Old JSON without `wall_settings` loads with defaults; validation and the first
  Apply upgrade the element to an explicit settings record.

### Model effects implemented now

| Setting | Effect |
|---|---|
| Geometry method | Selects the Wall tool's drawing state machine for the next wall. Changing it cancels an unfinished draft. Editing an existing wall stores the value but does not convert its geometry. |
| Height, thickness | Update the wall shape; Plan and 3D use the same shape. |
| Home Story, layer, renovation status | Update the element and its layer/renovation filtering. |
| Start/end X/Y | Update the wall reference line (and therefore the fixed blue baseline) and all derived views. |
| Reference line location and reference offset | `Center` keeps the body centred on the reference line; `Inside`/`Outside` offset the body by half the thickness to the respective side, and the numeric `Reference Offset` is added on top. The blue reference line itself stays on the start/end anchors. |
| Top/bottom offset | Shift the 3D wall's top and bottom elevation. |
| All other groups (profile offsets, floor-plan display overrides, surfaces/texture, structural analytical parameters, classification and property values) | Typed, validated, persisted with the element, and restored on save/reopen. Physical or rendering effects are not claimed. |
| Name, ID and classification values | Shown in Properties and retained in JSON. |

### Explicitly not implemented

- Parametric profile generation from `Profile Offset Modifiers`; the values are
  metadata and do not change the rectangular wall solid.
- Floor-plan line/fill overrides (`Uncut Lines`, pen weights, cut-fill pens,
  wall-end line suppression) and projection-range clipping.
- Surface/texture material mapping; the renderer still uses the element
  `Material`.
- Top-link geometry solving. The AC29 top-link value is stored, while explicit
  height and top/bottom offsets drive the solid.
- Structural analytical model generation, support conditions, rule-based
  modification, brick takeoff and cost formulas.
- A single Polygon Wall element. Rectangular and Polygonal creation currently
  commit groups of ordinary wall segments; polygon-specific reference-edge,
  fill, self-intersection and pet-palette editing are not implemented.

## Reuse

`src/wall_ui.rs` owns the draft, validation, section state and AC29-style
collapsible body. The same module is used by the selected-wall dialog, the
wall-default dialog and the Properties summary/button. A future Wall tool
inside another workflow can construct `WallSettingsDialog` or
`WallSettingsDraft` without duplicating field definitions.

## Acceptance checklist

1. Select a wall and press `⌘T`: the dialog shows all six groups and current
   values; Cancel leaves the model, selection and undo history unchanged.
2. Change height, thickness, reference-line location (`Center`/`Inside`/
   `Outside`), reference offset, top/bottom offset, story and start/end
   position; Apply updates Plan and 3D while the blue baseline stays on the
   reference anchors; one Undo restores all values.
3. Save, reopen and confirm every group's values survive.
4. Activate the Wall tool with no selection, press `⌘T`, change defaults, Apply,
   then draw a wall: the new wall uses the new defaults.
5. Switch English → 简体中文 → English: section titles, field labels, choice
   values, notes and buttons follow the language; project and undo state do not
   change.
6. Restart after choosing 简体中文 and confirm the dialog opens in Chinese.

## Desktop verification (2026-10-18)

Final packaged app: `dist/OpenArchi.app`, `Courtyard House`, 12 elements,
Simplified Chinese after restoration.

- Target bundle freshness: `target/release/openarchi` and
  `dist/OpenArchi.app/Contents/MacOS/openarchi` both report UUID
  `9AAE1B31-4869-3AB8-97B0-176E92519638` (arm64).
  `codesign --verify --deep --strict dist/OpenArchi.app` exits 0. The release
  binary SHA-256 is
  `b9a71360b1e42506100e67970b35ceb698e3c16468ee4f43729e1ad097508f58`; the
  packaged executable differs because packaging applies an ad-hoc signature.
- Selected-wall edit: selected `Wall 005`, pressed `⌘T`, changed Height from
  `3.000 m` to `4.250 m`, clicked **Apply** and **OK**. The status line became
  `墙设置已应用 · 平面图与三维视图已同步`; the project showed its unsaved
  marker and `Undo` became enabled.
- Undo: one Undo returned Height to `3.000 m`, disabled Undo/Redo as
  appropriate, and reopened `⌘T` confirmed the persisted value was `3.000 m`.
- 3D synchronization: repeated the height edit in the 3D view. After OK the
  affected wall was visibly taller than the unchanged walls; one Undo restored
  the uniform height. This was a visual check, not a numerical geometry audit.
- Language coverage on the final bundle: switched Chinese → English from the
  title-bar selector, then English → 简体中文 from `View > Language`; also
  repeated the reverse direction on the restarted instance. The title, helper
  text, buttons and first-group labels/choices were read in both languages.
  Scrolling the dialog showed all six group headers in both languages:
  `Geometry and Positioning`, `Profile Offset Modifiers`, `Floor Plan and
  Section`, `Model`, `Structural Analytical Parameters`, `Classification and
  Properties`, and the corresponding Chinese titles.
- Language switch remained presentation-only in the observed run: selection
  stayed at one element, the count stayed at 12, and Undo/Redo stayed disabled
  both before and after switching.
- Restart: after choosing `简体中文`, quit and relaunch the packaged app; the
  first AX tree and screenshot were already Chinese, and the title-bar selector
  showed `简体中文`.

Limits: this desktop pass expanded and read the first group in both languages
and read all six group headers after scrolling. It did not individually edit
every field of the remaining five groups on the final bundle; their field
coverage is the typed draft, validation, persistence and regression tests
listed above. CUA captures have no REA Evidence ID. No Archicad 29 runtime
comparison was made for OpenArchi's own labels or undo behavior.

### Info Box wall-tool defaults (2026-10-18, final bundle)

Final packaged app: `dist/OpenArchi.app`, arm64 UUID
`C64008A3-1EA6-3CE7-8B11-0A469EF35FB6`, release SHA-256
`86a21e6be1b7a824332031b57332596e7df5479db3609dec55fc938a3e1a9ec3`;
`codesign --verify --deep --strict dist/OpenArchi.app` passes.

The desktop pass used a signed test copy of the immediately preceding build
with the same rendered strings; the only later change removed duplicate
same-value entries from the locale files and rebuilt the package.

- Wall tool active, nothing selected: the Info Box showed `默认设置`, layer
  `03 建筑`, geometry method, reference line, offset, structure, thickness,
  height, top link and both offsets.
- Edited thickness `0.220 → 0.360 m`, height `3.000 → 4.200 m`, reference line
  `外部`, and reference offset `0.150 m`. The values survived a Wall → Select →
  Wall tool switch, `撤销` stayed disabled and the count stayed at 12.
- `设置…` opened `墙默认设置` with all edited values. `⌘T` and
  `平面图与剖面…` share the same default-dialog path and are covered by
  regressions.
- Drawn wall `墙 013` (4.000 m, 90°) inherited layer `03 建筑`, reference line
  `外部`, offset `0.150 m`, thickness `0.360 m`, height `4.200 m`, top link
  `1. First floor`, and structure `Generic Wall/Shell`. The Info Box switched to
  the selected-wall form, the count became 13, and Undo was enabled.
- English tool-state pass (same-content test copy relaunched with preference
  `en`): the Info Box read `Wall`, `Default Settings`, `Layer:`, `Geometry
  Method:`, `Reference Line:`, `Reference Offset`, `Structure:`, `Thickness`,
  `Height`, `Top Link:`, `Top Offset`, `Bottom Offset`, `Floor Plan and
  Section…`, `Settings…`, `Apply`. Selecting `Outside` reported `Wall default
  settings applied · New walls use these values`; `Settings…` opened `Wall
  Default Settings` with `Reference Line: Outside`; the count stayed at 12 and
  `Undo`/`Redo` stayed disabled.

Limits: the desktop pass edited the reference/offset, thickness and height
directly; layer and top-link dropdown changes, structure-name typing, invalid
input, and every field of the default dialog were covered by unit/render
regressions but not re-typed on this packaged instance. CUA popup clicks were
unstable for egui combo menus, so the layer selection was not changed in the
desktop run.
