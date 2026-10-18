# Wall reference-line editing — Archicad 29 / OpenArchi

## Goal

Edit a selected straight wall from its reference line (baseline), using the
same model as Polyline editing: the stored line is the editable truth, while
the wall body, joins, 3D geometry, Trace output and hosted openings are derived
from it.

Target: Archicad 29 `29.0.0 v1 INT (5101)`, arm64. OpenArchi uses metres and
world-axis plan coordinates.

## Evidence

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| Wall start and end coordinates are independently replaceable and stored as points on the wall | static artifact analysis | `VBElem::Wall::SetBegC(...)` at `VBElements +0x858448`; `VBElem::Wall::SetEndC(...)` at `VBElements +0x858458`; framework SHA-256 `f86d144f5f04ce1c7a88a2df831c4b54459f3ad703be53768999a24aab70aae3` | Both symbols store a `Point2<double>` and clear a dirty flag. This establishes two editable endpoint setters, not their complete UI interaction. |
| Archicad exposes distinct wall node-editing interactions for the two reference-line ends | static artifact analysis | `WallEditingInteractions::GetEditNodes(...)` at `VBElemEditingInteractions +0x40138c`; `WallEditingInteractions::GetAvailableNodeArray(...)` at `0x40e514`; `WallNodeEditingInteraction<0..8>` symbols from `0x40f5b0` | Symbol and dispatch inventory only. Individual drag rules, snapping and linked-wall behavior were not recovered. |
| A wall body is generated from its reference line, a lateral offset and width | static artifact analysis | `VBElem::Wall::RectPoly` at `VBElements +0x84e1b4`; `VBElem::Wall::GetCorePolygon` at `0x84db0c` | Straight-segment branch; curved walls are outside this slice. |
| The visible join outline is derived from connection data at display time | static artifact analysis | `VBD::WallVRDData::CalculateWPolygon` in `VBElemOperations`, plus the calls recorded in [wall-joins.md](wall-joins.md) | OpenArchi reproduces the visible straight-wall result geometrically; it does not claim the same internal connection records. |
| Dragging a selected baseline endpoint reshapes that end; dragging the baseline moves the wall | product requirement and interaction design | This task; Archicad 29 selection capture described in [wall-selection.md](wall-selection.md) | No AC29 runtime capture of the drag itself and no REA Evidence ID. The design is consistent with the observed blue reference line and black square handles. |
| Wall endpoint editing generates point, line and surface neighbour candidates and has a dedicated reference-line check | static artifact analysis | `VBEI::Wall::WallEditingInteractions::ReferenceLineNeeded(...)` at `VBElemEditingInteractions +0x409eb0`; `IncreasePriorityOfPoints` constructor at `0x40a060` with `AddPoint` `0x404be4`, `AddLine` `0x404cd8` and `AddSurface` `0x404ce8`; framework SHA-256 `0550c7c937e513d0082cc8cb902c461b47efb96357077beffe82eabc76e75936` | The symbol inventory proves an endpoint/line/surface candidate mechanism and a reference-line check; it does not establish the exact snap radius, priority order, release commit rule or join trim. No REA Evidence ID. |

No Archicad source, private resource or branded asset is copied. The model
operation, preview state and drawing code are original OpenArchi code.

## Interaction contract

- With Select active and a wall selected, the blue reference line and its two
  square handles are editable. A handle hit is tested before the line segment,
  so a press near an end of a thick wall starts endpoint editing rather than
  moving the whole wall.
- Press-drag a start or end handle to move only that stored endpoint. The other
  endpoint remains fixed and the wall length and angle follow the pointer.
- While a start or end handle is dragged, a visible same-story straight wall
  whose reference line is within `12 px` **or whose body contains the pointer /
  lies within `12 px` of it** becomes a magnetic target. The stored endpoint is
  set to the perpendicular projection clamped to that wall's reference segment,
  so the wall meets the neighbour's baseline exactly when the pointer is
  released. The two proximity tests are independent: a reference offset that
  shifts the body away from the line still leaves the baseline capturable. This
  makes the visible body of a thick wall draggable, not only a pointer position
  close to its centre line. The target is drawn in accent colour with a snap
  marker and a localized `Snap: wall reference line` label. Releasing inside
  the region resolves the snap once more, so release without a final
  pointer-move frame still commits the intersection.
- Baseline capture is evaluated from the raw pointer position before grid or
  object snapping. Grid snapping may still quantize the endpoint along the
  edited wall's direction, but it cannot pull the pointer away from a captured
  baseline (for example, a `1 m` grid must not turn a `2.4 m` baseline hit into
  `2.0 m`). This keeps endpoint handoff reliable with coarse drawing grids.
- Whole-reference-line moves never snap; they preserve length and orientation.
- Press-drag the reference-line segment away from either handle to translate
  both endpoints by the same delta, moving the whole wall.
- Click-release on a handle or the line opens a small palette identifying the
  start endpoint, end endpoint or whole reference line and showing the current
  length. The palette does not create a second editing model.
- A live drag displays the edited wall through the ordinary Plan renderer. The
  preview is recomputed from the gesture-start project, so repeated pointer
  moves do not accumulate rounding or earlier preview values.
- Releasing a changed drag commits exactly one undo transaction. The selection
  and wall identity remain unchanged. Clicking **Done** on an unchanged palette
  only closes it and returns the status bar to the Select prompt.
- Escape during a live drag restores the gesture-start project and adds no undo
  entry. Escape while the idle palette is open closes the wall edit through the
  existing command-cancel path.
- Leaving Select or starting another selection/edit clears `wall_edit`. A
  committed drag closes the edit automatically.
- A snapped endpoint is still an ordinary reference-line edit: the model stores
  only the new endpoint, while `plan_wall_footprints` trims the body against the
  target's near face for a T join (or mitres an L join) and the same footprint
  feeds Plan, 3D and Trace. If the snap would make the wall shorter than the
  minimum or strand a hosted opening, validation rejects it like any other
  endpoint position.
- Reference-line validation rejects non-finite or out-of-range points and walls
  shorter than `0.01 m`. It also rejects a change that would put any hosted
  opening partly outside the host wall. Invalid preview positions leave the
  last valid preview in place and show the reason in the palette; releasing
  only commits a valid changed preview.

## Derived geometry contract

The only mutation is `Shape::Wall { start, end }`. Everything else is recomputed
from the project:

- `geometry::footprint` derives the wall body from the edited line, thickness,
  reference-line location and reference offset.
- `geometry::plan_wall_footprints` derives corner, offset-corner and T joins
  from the new reference lines. Plan fill, Plan outlines, Trace and ordinary 3D
  consume the same joined footprints.
- Hosted openings keep their stored offset along the named `start → end`
  direction. Their absolute placement and the opening cut follow the edited
  host line automatically.
- Project JSON stores only the wall endpoints and existing wall settings; no
  preview or join geometry is persisted.

## Model operation

`Project::validate_wall_reference_line(id, start, end)` performs validation
without mutation. `Editor::set_wall_reference_line(id, start, end)` validates,
then replaces both endpoints. It intentionally does not call `commit`: the
interactive caller previews against the project snapshot captured at gesture
start and calls `Editor::commit` once on release. Non-UI callers can therefore
group a baseline edit with their own transaction.

## Reversible assumptions and exclusions

- Automatic reference-line capture applies only to straight walls on the active
  story whose Plan visibility is on, and excludes the wall being edited. Curved
  walls and walls hidden by layer/visibility settings are not snap targets.
- The snap radius is a fixed 12 screen pixels converted through `plan_scale`;
  AC29's exact snap priority and distance thresholds were not observed. The
  radius is measured from the raw pointer, not from a grid- or object-snapped
  position.
- Adjacent walls are not automatically moved when an endpoint is edited. The
  join outline is recomputed and may extend into or stop at the neighbour, but
  a shared endpoint on a different wall remains independently owned. Linked
  wall endpoint following has no AC29 runtime or source evidence in this pass.
- Moving one endpoint past the other reverses the stored `start → end`
  direction. Hosted openings retain their numeric offset from the named start,
  so their absolute position changes with that reversal. AC29 behavior for this
  case was not observed.
- The palette is intentionally informational. It does not yet expose AC29's
  full node-editing command set, curved walls, arc conversion, wall-profile
  editing or linked-wall controls.
- Group, locked, multi-story and composite walls are not specially handled.
  Multiple selected straight walls can each be edited, but only the nearest
  handle or line under the pointer is claimed.

## Acceptance checklist

1. Select a straight wall. Its blue reference line ends in two square handles.
2. Drag each handle separately. Only that stored endpoint moves; the body,
   length label, joins, 3D and hosted openings update from the new line.
3. Drag an endpoint near another wall's blue reference line. The target line
   and snap point highlight; release stores the exact projected point and the
   derived footprint trims at the neighbour's face in Plan and 3D.
4. Drag the middle of the reference line. Both endpoints move by one delta.
5. Release each drag. One Undo restores the complete gesture-start state and
   one Redo reapplies it.
6. Press Escape during a live drag. Geometry returns to the gesture-start state
   and Undo/Redo history is unchanged.
7. Shorten a wall so a door or window would leave its host. The palette reports
   the rejection and the wall keeps the last valid geometry.
8. Save, reopen and inspect the wall. The edited endpoints and derived joins
   are restored without extra preview data.
9. Exercise the palette, snap label and status in English and Simplified Chinese.

## Automated coverage

- `app::ui_tests::wall_endpoint_handle_drag_stretches_only_that_endpoint_once`
- `app::ui_tests::wall_baseline_drag_moves_the_whole_reference_line`
- `app::ui_tests::wall_endpoint_drag_snaps_to_another_reference_line_and_joins`
- `app::ui_tests::wall_endpoint_drag_onto_a_foreign_material_wall_body_snaps_and_joins`
- `app::ui_tests::wall_endpoint_release_inside_snap_radius_joins_without_pointer_move`
- `app::ui_tests::wall_endpoint_reference_snap_wins_over_a_coarse_grid_snap`
- `app::ui_tests::wall_endpoint_snap_preview_matches_the_active_language`
- `geometry::marquee_tests::wall_reference_snap_projects_onto_another_baseline_and_skips_the_edited_wall`
- `geometry::marquee_tests::wall_reference_snap_accepts_a_pointer_inside_a_thick_foreign_material_wall`
- `geometry::marquee_tests::wall_reference_snap_accepts_the_baseline_when_the_body_is_offset_away`
- `geometry::marquee_tests::cross_material_and_renovation_t_junction_trims_caps_and_outline`
- `app::ui_tests::wall_baseline_click_opens_endpoint_or_line_edit_and_escape_closes_it`
- `app::ui_tests::wall_reference_line_palette_renders_in_both_languages`
- `app::ui_tests::wall_palette_keeps_the_edit_when_the_canvas_is_under_the_popup`
- `app::ui_tests::escape_cancels_a_live_wall_baseline_drag_without_undo`
- `app::ui_tests::wall_baseline_edit_refuses_to_strand_a_hosted_opening`
- `workflows::wall_reference_line_edit_drives_geometry_and_roundtrips`

## Desktop check

On the current final bundle `E6E06421-C90A-3A80-B072-DFBD824F28CF`, with
isolated HOME `/tmp/oa-drag-test/home` and project `Wall Join Final`
(`Wall 001=(0,0)→(4,0)`, `0.3 m`; `Wall 002=(2,2)→(2,-2)`, `1.0 m`), the
selected `Wall 001` start handle was dragged from `(0,0)` to the `Wall 002`
baseline at `x=2`. CUA could not produce a press → multi-frame move → release
sequence, so the gesture was delivered as real CGEvent mouse events (press,
50 drag frames, release). The Chinese run showed `捕捉：墙参考线` during the
drag and `墙端点已连接到参考线` on release, stored `(2,0)`, trimmed the plan
body at `x=2.5`, and restored the original cross wall with one Undo; Redo
reapplied it and 3D showed no buried end cap. Screenshots:
`/tmp/oa-drag-test/screen13-drag.png`, `screen14-zh-result.png`,
`screen7.png`, `screen8.png`. The English status text was also observed as
`Wall endpoint joined at reference line` (`screen6.png`).

## Remaining unknowns

- No AC29 runtime observation of endpoint or baseline dragging; the static
  symbol evidence and selected-wall capture do not establish the exact AC29
  snap radius, priority order, cursor glyph, release commit rule or undo
  behavior. OpenArchi's 12 px endpoint-only capture is a deliberate,
  reversible approximation.
- The neighboring linked wall endpoint behavior, wall-direction reversal,
  curved walls and wall-profile bodies remain unverified or unimplemented.
- The current drag does not offer a numeric length/angle editor in the wall
  palette; the Plan Tracker remains part of wall creation rather than this edit.
