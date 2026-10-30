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
- Historical initial palette was informational (superseded by the 2026-10-29 insertion update below). It did not expose AC29's
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


## 2026-10-29 selected wall middle-edge palette

User screenshot shows Insert new node active, a stationary original segment and a two-leg preview. Goal: after rotating the wall or observing it from a rotated view, left-click a selected wall's middle reference segment and perform actual shape editing from its palette. View clarification is pending; implement the shared straight-wall node operation independently first. The existing palette is informational and 3D has selection/orbit only (readable source: `open_wall_edit`, `wall_edit_palette`, `model_view`).

Evidence: user screenshot for visible palette/preview only; [AC29 Adding Element Nodes](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-90.htm) documents select→edge click→Insert new node→place and creation of an additional element for straight linear elements. It explicitly restricts 3D node insertion to polygonal elements. [Pet Palettes](https://help.graphisoft.com/AC/29/INT/_AC29_Help/020_Configuration/020_Configuration-27.htm) documents context-dependent commands. No AC runtime claim this turn.

Contract: keep original endpoints, preview two connected straight wall pieces through the new point, click/Enter to commit one transaction, Escape cancels. Use a private validated project candidate, original material/layer/wall settings/story and identity on first segment, new identity on second; openings wholly before/after the original hit station retain their measured offset on their respective piece, splitting through an opening rejects atomically. Lock and group policy follows existing reshape. Plan/3D/joins derive from those same two walls and project JSON stores them normally. Numeric distance in metres and angle in degrees are optional; label units explicitly. Preserve existing direct endpoint/whole-wall dragging. Palette exposes supported operations only; curved-wall, slanted/profile commands remain outside the current straight-wall model. If the 3D route is implemented, straight-wall insertion there is an explicit OpenArchi extension, not exact AC29 equivalence.

Acceptance: rotated wall and rotated plan hit-testing; original endpoints fixed and node shared; palette pointer input isolation; preview/cancel no model/history mutation; locked/group/invalid/opening rollback; material and opening remapping; actual UI command click and point completion; undo/redo and JSON roundtrip; both language packs and final native package.


### Implemented slice and current evidence

View clarification was not answered during work. Both rotated-plan and 3D projected-reference-edge routes were implemented with one shared model operation. The 3D straight-wall route is an explicit OpenArchi extension (AC29 help limits 3D node insertion to polygonal elements). This update implements the screenshot's highlighted **Insert node**, plus palette entry to existing Drag/Rotate/Mirror/Multiply commands; it does not claim all screenshot icons, curved-wall or complex-profile editing.

- `ReshapeCommand::InsertWallNode` / `reshape::candidate`: validated private candidate, split at the original clicked station, preserve source endpoints/metadata, first ID retained/second ID allocated, opening host/offset remapped; a split through an opening and locked affected openings reject. Suspended group membership extends to the second segment. Commit uses the existing single-undo editor operation.
- `wall_node_ui::WallInsert`, `start_wall_insert`, `wall_insert_canvas`: preview-only state, point placement, Distance in metres / Angle in world XY degrees, Apply/Enter, Cancel/Escape; numeric input locks the preview until Follow pointer. Actual original model is untouched before confirmation. The first palette includes supported transform routes only.
- `set_wall_edit_hit` uses the clicked station, not the unrotated screen midpoint. Plan projection honors orientation. `selected_wall_model_edge` projects base/top reference segments and unprojects input onto the same horizontal elevation plane; orbiting while an insertion is pending cannot claim the primary pointer. Existing ordinary orbit/plan drag remain available. 3D selection now displays the top reference line/handles. Node-end dragging in 3D was not added; this entry is specifically the middle segment.
- Endpoints and 3D/plan geometry remain derived from normal wall elements, with no inserted-node special persistence format. Adjacent unrelated walls are not moved.

### 2026-10-29 final verification

Forced verification `target/ac29-workflow/runs/20261028T183751Z-e2a64a9f/report.json`: **377 Rust tests + 15 tooling tests passed**, formatting/includes, cargo check all targets, locale validation, diff and packaging passed. Four new tests cover rotated plan + both languages + actual palette button/canvas input + preview/cancel/undo; orbit then 3D edge hit + placement without camera drift; exact endpoints/metadata/opening remap/JSON roundtrip; invalid/locked/group/opening atomic rejection. Existing wall dragging and Cmd Trim tests also passed in the full suite.

Final app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **1BA174C1-BFB7-3218-8144-D1F78A68D3F2**. Strict deep codesign passed; `bundle` recheck matches successful source receipt.

Native smoke on final package: isolated HOME `target/wall-node-smoke-home`, initial PID 99920/window 42013, later restart PIDs 22057/35684. User's PID 99546 untouched. Foreground-guarded CGEvents for canvas/orbit and AXPress for controls/native menus. [Fixture](wall-node-evidence/fixture.json): one straight wall (-3,-1)→(3,1), thickness 0.3 m, height 3 m.

| Claim | Result / evidence |
|---|---|
| Rotated wall middle click opens actionable palette | pass: screen (895,562); [EN palette](wall-node-evidence/plan-palette-en.png), AX text alongside |
| Pointer preview leaves model unchanged | pass: before-click recovery JSON equals fixture; [EN preview](wall-node-evidence/plan-preview-en.png) |
| Plan click creates two connected walls with fixed original endpoints | pass: node (-1.0883694,-1.9953438), count 1→2; [saved JSON](wall-node-evidence/plan-result.json), [screen](wall-node-evidence/plan-result.png) |
| Undo/Redo | pass: native menu Undo/Redo/Undo restored 1/2/1 walls |
| Orbit, then click projected 3D middle reference segment | pass: native left drag (1100,700)→(1160,715), then click (883,469); [orbited](wall-node-evidence/model-orbited.png), [ZH palette](wall-node-evidence/model-palette-zh.png) |
| 3D preview/cancel/commit | pass: [ZH preview](wall-node-evidence/model-preview-zh.png); Cancel recovery JSON equals fixture; confirmed node (-1.1109972,-2.0351205), [JSON](wall-node-evidence/model-result.json), [screen](wall-node-evidence/model-result-zh.png). Undo/Redo/Undo passed |
| Editable numerical fields | pass: AXSetValue Distance=2.5 m and Angle=90°, [AX readback](wall-node-evidence/numeric-locked-ax.txt); palette Apply produced node (-0.0021768708,2.4992743), original clicked anchor≈(-0.0021768,-0.0007257), [JSON](wall-node-evidence/numeric-result.json). Initial AX text-match attempts encountered changed live values; final retry used the observed popup's last two AXIncrementors. Duplicate Apply labels were disambiguated to the popup button; failed attempts made no commit |
| Bilingual surfaces and restart | pass: native View > Language EN→ZH→EN; main-window title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar, idle and active insertion palettes, numeric labels inspected. [ZH numeric](wall-node-evidence/numeric-zh.png), [EN numeric](wall-node-evidence/numeric-en.png). Chinese restart first-frame title `未命名`; [capture](wall-node-evidence/final-chinese-restart.png), recovery reopens two-wall result [AX](wall-node-evidence/reopened-ax.txt) |

Isolated preference restored to English; all test processes closed. Title-bar language selector remains absent per prior product configuration. Floating Quick Options, transform dialogs reached through the new aliases, curved/slanted/profile walls, native hosted-opening splitting and arbitrary occluded 3D edges were not desktop-tested. Native persistence used recovery/reopen, not Save As. Exact dirty/undo stack invariance across language switches remains covered by existing automated localization tests, not extracted from this native process. No complete AC29 or full-localization equivalence claimed.

Candidate reusable lessons (unconfirmed; not added to AGENTS.md):
- Project and unproject a 3D editable edge on the same elevation plane; screen midpoint arithmetic alone cannot recover the correct world hit after orbit.
- Topology edits should preview private candidates and commit once, remapping hosted elements before validation; a split through an opening must reject atomically.

## 2026-10-29 — User correction: full icon palette

Supersedes the single Insert-node/text-popup scope above. Middle-edge clicks now offer all ten commands as an original two-row icon palette, with shared pending state and a separate tracker. Exact arc and trapezoid contours persist and drive plan/3D, picking and hosted openings. [Contract and supported workflows](wall-pet-palette/contract.md); [final 399-test/native ten-command acceptance](wall-pet-palette/validation.md). Custom-wall junction cleanup and unsupported legacy partial edits remain bounded as documented.

## 2026-10-30 — endpoint palette unified

Current maintainer instruction replaces the old endpoint text popup with the same ten-icon/two-row palette used by Insert node. Endpoint first slot means Move node; other commands reuse the existing shared pet session. Switching back restores the original endpoint drag, validation and reference-line snapping path. Left-click activates a command; hover never switches it. Escape dismisses; passive length text stays removed. No endpoint geometry or 3D entry semantics changed. Both endpoint/language event regressions and current native English/Chinese start-endpoint inspection passed; see [contract and current verification](wall-pet-palette/contract.md#endpoint-menu-delivered-and-verified--2026-10-30).
