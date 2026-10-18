# Grids menu — Archicad 29 reference / OpenArchi implementation

## Scope and evidence (2026-10-17)

Goal: reconstruct the Archicad 29 Standard Toolbar `Grids` pop-up and its
`Grids & Background` settings in the independent native OpenArchi model.

Reference target: Archicad 29.0.0 v1 INT (5101), arm64 macOS,
`/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app`, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`
(reused from `scripts/ac29-workflow.py context grids`).

Runtime observation (CUA, current Archicad process, Untitled project, Ground
Floor plan, Arrow tool):

- Standard Toolbar `Grids` pop-up contains, in order: Construction Grid
  Display; Grid Snap; Snap to Snap Grid; Snap to Construction Grid; Origin
  Display; Orthogonal Grid; Rotated Grid; Set Rotated Grid; Grids & Background…;
  Editing Plane Display; Pick Plane; Horizontal; Offset; Snap to Story….
  The four editing-plane commands after `Grids & Background…` were disabled in
  the captured floor-plan state.
- `Grids & Background…` opened a modal with: Show Construction Grid; Main Grid
  and Aux. Grid (Spacing and Steps for both X and Y); Rotate Grid; Local Origin
  X/Y; rotation angle (45.00° default); Snap Grid / Nudge Spacing X/Y (50 mm
  default); three snap-state buttons; Background and Grid Lines color swatches
  with sliders; a preview; preview zoom (25%) with zoom buttons; Cancel/OK.
  The dialog was cancelled after observation; no project setting was applied.
- A temporary draft of Main 2000/2000 steps 1/1 and Aux. 1000/1000 steps 2/2
  was previewed at 200% and the dialog was then cancelled. Main and auxiliary
  lines are visually the same color in the preview. The preview cannot prove
  Archicad's exact line phase, so OpenArchi uses the documented cycle below.

Public documentation (AC29):

- [Set up Construction Grid](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-43.htm)
- [Set up Snap Grid](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-44.htm)
- [Snap to Grid](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-45.htm)
- [Nudging Elements](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-70.htm)

Documented semantics used here:

- Main and auxiliary grid values are pairs of Spacing (X/Y) and Steps (X/Y).
  Steps are the number of lines of that kind shown before switching to the
  other kind; a Steps value of 0 hides that kind. Example: one main line
  (spacing 2000) then two auxiliary lines (spacing 1000), then repeat.
- The grid hides itself when adjacent lines would be closer than 3 screen
  pixels. The dialog preview uses a separate zoom percentage.
- Snap Grid is invisible and snaps to its own X/Y spacing. Construction Grid
  snapping snaps to construction-grid nodes. The three states are No snap,
  Snap to Snap Grid and Snap to Construction Grid (Shift+S cycles them); Grid
  Snap (Alt+S) toggles snapping while remembering the last mode.
- Nudge uses Snap Grid/Nudge spacing; Shift+Nudge uses construction-grid
  spacing. Rotating the grid uses a local origin and an angle; `Set Rotated
  Grid` derives the angle from a drawn vector or an existing straight segment.
- Grid settings are shared by all stories in the floor plan. Archicad stores
  them per view; OpenArchi stores one floor-plan set in the project document.

## Contract / reversible implementation choices

- `Project.grid_settings` is a typed, serialized `GridSettings` with
  backward-compatible defaults. Units are metres in OpenArchi (Archicad's
  dialog uses working units, observed here as millimetres). Defaults mirror the
  observed dialog: Main 2.000 m, Steps 1; Aux. 0, Steps 0; rotation 45°; Snap
  Grid/Nudge 0.050 m. Construction Grid Display, Grid Snap and the active snap
  mode are all off by default (`No snap`), so a new or legacy project without
  saved grid settings opens on a clean drawing canvas.
- The grid is a repeating cycle of `(spacing, kind)` advances. The origin is a
  line. Forward: `main_steps` main intervals, then `aux_steps` auxiliary
  intervals. Backward: the same cycle reversed. This reproduces the documented
  one-main/two-auxiliary example (offsets 0 main, 2 aux, 3 aux, 4 main at the
  default 2 m/1 m values).
- Main/auxiliary lines use the Grid Lines color, with the auxiliary line drawn
  at lower alpha. Background color applies to the floor-plan paper only.
- Construction-grid snapping resolves in grid-local coordinates, so rotated
  grids snap correctly. Snap-grid snapping rounds local X/Y to the snap
  spacing. Element endpoint/midpoint snapping keeps priority; grid snapping is
  the fallback when element snapping does not resolve.
- `Set Rotated Grid` is a two-click vector tool; a first click on a straight
  wall/slab/polyline segment takes that segment's angle immediately. Escape
  cancels. The command enables rotation and the rotated display.
- Dialog edits are a draft. Cancel/Escape leave saved settings unchanged. OK
  validates and applies one undoable project-settings transaction. Toolbar
  display/snap toggles are view-state edits and are not added to undo history.
- Both plan and 3D derive lines from `Project.grid_settings`. The 3D grid is
  drawn on the ground plane (lowest visible story minus 2 cm) over the model
  bounds plus padding, using the configured rotation and Grid Lines color; the
  auxiliary line is blended toward white instead of using alpha. Editing-plane
  commands (Pick Plane, Horizontal, Offset, Snap to Story…) are present but
  disabled with an explanation because OpenArchi has no editing-plane model.
  Per-view grid variants, section/elevation grids and the structural Grid Tool
  are outside this slice.
- Dialog widgets edit one persistent draft stored in the application. Moving
  the pointer or repainting does not reset values. `Rotate Grid` previews the
  configured angle at once while the menu's Orthogonal/Rotated choice still
  controls the saved display mode. The opening menu click is ignored by
  `Modal::should_close` for one frame so the dialog is not dismissed by the
  click that opened it.

## Acceptance checklist

1. Open `Grids`; every item is present, view/snap toggles change the canvas,
   and `Grids & Background…` opens.
2. Main 2 m / Steps 1 with Aux. 0 draws a 2 m grid; Aux. 1 m / Steps 2 produces
   the documented cycle; Steps 0 hides that kind.
3. Rotate Grid + rotated display draws the grid about the local origin at the
   angle; Set Rotated Grid derives and applies an angle from a segment/vector;
   Escape cancels.
4. Snap Grid at 0.05 m snaps to 50 mm nodes; Construction Grid snaps to the
   nearest main/aux intersection; rotating the grid rotates both.
5. Shift+S cycles no/snap/construction; Alt+S toggles the last mode; arrow
   nudge uses snap spacing and Shift+arrow uses construction spacing.
6. Invalid spacing/steps are rejected in the dialog without changing project
   data; OK applies once and is undoable; Cancel leaves settings untouched;
   JSON save/load preserves the settings.
7. The packaged app is rebuilt and a native smoke test is attempted.

## Default-state update (2026-10-18)

- New and legacy projects that omit `grid_settings` start with the construction
  grid hidden, grid snapping disabled and `GridSnapMode::None`.
- Turning Grid Snap on from the toolbar or `Alt+S` still restores the normal
  Snap Grid mode; the default being off does not remove the remembered-mode
  behavior.
- `grids::tests::construction_grid_and_snap_are_off_by_default` covers the
  new default. Existing snapping tests now enable the mode explicitly.

## Verification (2026-10-17)

- `python3 scripts/ac29-workflow.py verify` passed formatting (including
  standalone formatting of `src/grids_ui.rs`), all-target `cargo check`,
  **132 Rust tests**, 11 tooling tests, `git diff --check` and
  `./scripts/package-macos.sh`.
- Deliverable: `dist/OpenArchi.app`; release/bundle arm64 UUID
  `23A2BCE9-1470-3087-A872-755B2F66F1CC`; strict deep signature verification
  passed.
- Native desktop smoke on the packaged app (OpenArchi 0.1, Simplified Chinese,
  demo project): opened the toolbar `网格` menu and confirmed the 14 items and
  order, with the four editing-plane commands after `网格与背景…` disabled;
  opened the dialog and dismissed it with Escape; reopened it, toggled
  `构造网格显示` off and on, toggled `旋转网格` and saw the 25% preview switch
  to a 45° grid, selected `不捕捉`, and activated `确定`; observed
  `Grids & Background applied`, a dirty title, an enabled Undo entry and the
  unrotated plan grid; Undo restored the prior settings, cleared the dirty
  marker and disabled Undo. CUA pointer clicks inside the egui modal were not
  delivered reliably, so the dialog controls were exercised through Tab focus,
  Space and Return; this is a tooling limitation, not an application path.
- Desktop not exercised: numeric field typing through CUA (DragValue text
  entry did not receive CUA keystrokes), pointer clicks on modal controls,
  Shift+S / Alt+S, arrow nudging, `Set Rotated Grid`, outside-click dismissal
  and JSON save/reopen. Those rely on unit/RawInput coverage and the contract
  above, not on a desktop pass.

## Known gaps

- The exact Archicad line phase and preview palette were not measured.
- Per-view grid sets, section/elevation/3D-document grids and Grid Tool
  structural grids are not implemented.
- Editing plane display, picking, horizontal offset and story snapping remain
  unavailable.
- Numeric entry and the shortcuts listed above still need a manual desktop
  pass; 3D auxiliary-line alpha is approximated by blending.

Desktop check on bundle `D0FD5583-C853-3379-9E3F-50329DF4C803`
(isolated HOME `/tmp/oa-join-final.AFg79r/home`, recovered project
`Same Material Join`): the 2D plan opened with no construction-grid lines
visible; two empty canvas regions (x=1760–2320 and x=650–2320 at
y=1400–1700 in `/tmp/join-recovered.png`) contain 0 non-background pixels.
The recovered project document reports
`show_construction_grid=false`, `grid_snap_enabled=false` and
`snap_mode=None`, matching `construction_grid_and_snap_are_off_by_default`.
The Snap Options checkbox itself was not read from the View menu in this
pass because the custom menu could not be opened reliably through the
automation; the off state is evidenced by the serialized project state,
the visible canvas and the regression test.
