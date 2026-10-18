# Wall joins and reference-line geometry — Archicad 29 / OpenArchi

## Scope

Goal: replace the recurring "wall corners look wrong" defect with a real
derived-geometry pipeline. The stored model keeps a wall as a reference line
plus thickness, reference-line location and numeric reference offset. Plan,
3D, Trace and 3D end-cap hiding must all derive the joined footprint from that
same data. The Wall tool must also end a draft on right-click and when the
start point is clicked again.

Target: Archicad 29 `29.0.0 v1 INT (5101)`, arm64, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`
(`python3 scripts/ac29-workflow.py context walls`). OpenArchi uses metres and
world-axis plan coordinates.

## Evidence

| Claim | Type | Source | Limits |
|---|---|---|---|
| A wall body is a trapezoid generated from the two reference-line endpoints, a per-end lateral offset and a per-end cross-section width; the reference line itself is not moved | static artifact analysis | `VBElem::Wall::RectPoly` at `VBElements` offset `0x84e1b4`; caller `VBElem::Wall::GetCorePolygon` at `0x84db0c` | Straight-segment branch only. The curved branch (arc height) was not reconstructed. |
| The per-end lateral offset is the offset of one body face from the reference line and the body spans `[offset, offset + width]` | static artifact analysis | `VBElem::Wall::GetWOffsetBeg` at `VBElements` offset `0x848b9c`: `±offset ∓ width ± profileMaxX` | The profile (`GetPrXMax`) branch was read but not exhaustively traced. |
| Joined wall outlines are derived at display time from connection data, not stored per wall | static artifact analysis | `VBD::WallVRDData::CalculateWPolygon` `0x34f06c` → `SetupAndStartWPolyCalculation` `0x34fc68` → `VBCalculations::MakeWall2PatStruct` `0x625bc`, all in `VBElemOperations` | Display path only; the on-disk wall record was not decoded. |
| A join replaces an end edge with points taken from the adjoining wall's polygon; two independent indices per end are chosen by `GetTind`/`GetCrossInd` | static artifact analysis | `VBCalculations::SetWall2PointAndInd` `0x369538` (writes four `Point2` corners from `CorePolygon2D` vertex indices), `VBCalculations::GetTind` `0x369a90`, `GetCrossInd` `0x369ef0` | Function identities and data flow observed; the exact index-selection branches were not fully reconstructed. |
| Walls are joined per connection end, and the connection set is chosen per wall from the wall connection list | static artifact analysis | `VBD::ChooseWall2Conns` `0x355944`, `VBD::GetBestWall2Ind` `0x354410`, `VBElem::WallConnectionList` | Same functions were located but not decompiled line by line. |
| Right-click finishes the current input; clicking the start point closes the contour | direct user instruction (this task) | — | Not re-observed in AC29 during this pass; OpenArchi desktop checks are recorded below. |
| Endpoint editing can generate point, line and surface neighbour candidates and checks whether a reference line is needed | static artifact analysis | `VBEI::Wall::WallEditingInteractions::ReferenceLineNeeded` `0x409eb0`; `IncreasePriorityOfPoints` `0x40a060` with `AddPoint` `0x404be4`, `AddLine` `0x404cd8`, `AddSurface` `0x404ce8` | Symbol inventory only; exact AC29 snap ordering, radius and release semantics remain unknown and are not claimed. |

Framework digests used for the artifact claims:

- `Contents/Frameworks/VBElemOperations.framework/Versions/A/VBElemOperations`
  SHA-256 `32c9d2c0014aca680d425aa31e17cdba84983fbaf287a9b15f7966f989ec304f`
- `Contents/Frameworks/VBElements.framework/Versions/A/VBElements`
  SHA-256 `f86d144f5f04ce1c7a88a2df831c4b54459f3ad703be53768999a24aab70aae3`
- `Contents/Frameworks/VBElemEditingInteractions.framework/Versions/A/VBElemEditingInteractions`
  SHA-256 `0550c7c937e513d0082cc8cb902c461b47efb96357077beffe82eabc76e75936`

No REA Evidence ID exists for these reads: the Hopper adapter that had been
opened for the previous investigation failed with
`ProviderAdapterError ... hopper adapter failed during open_binary`, so the
frameworks were disassembled directly with `llvm-objdump` from the installed
bundle. The digests above bind the claims to the exact bytes inspected.

### OpenArchi desktop verification

Desktop checks used `dist/OpenArchi.app`, isolated test HOME, project
`Courtyard House` with 12 starting elements. The bundle for these checks is
`49906133-4742-3E4F-9C8C-355537C01714`. The current final bundle is
`E6E06421-C90A-3A80-B072-DFBD824F28CF`; it keeps the wall-creation and join
geometry and adds independent baseline/body capture plus raw-pointer endpoint
capture (see `docs/validation.md` for hashes, checks and the explicit desktop
drag unknown). The same-material outline fix is packaged as
`D0FD5583-C853-3379-9E3F-50329DF4C803` and was checked on that bundle.

| Check | Result |
|---|---|
| Continuous, two nodes, Enter finishes | pass on the preceding bundle `D030826E-8720-3F87-9D4A-6E357BB89074` with identical geometry source: 12 → 13 elements, status `已创建 1 段连续墙`, Undo enabled |
| Continuous, two nodes, click the start point | pass on the same bundle: 12 → 14 elements, status `已创建 1 段连续墙`; no zero-length segment was added |
| Continuous, two nodes, right-click | pass on `C8ADA7AE`, which has the same wall-creation source: 12 → 13 elements, status `Created 1 connected wall segments`, Undo enabled, tracker and start marker cleared |
| Right-click with only a Single start point | pass on the preceding bundle: draft cancelled, status `墙绘制已取消`, no model or undo change |
| Status hint names the gesture | pass on `C8ADA7AE`: English `right-click, double-click or Enter to finish`; Chinese `右键、双击或回车完成` |
| Endpoint dragged onto another wall's reference line (desktop gesture) | **pass** on current bundle `E6E06421-C90A-3A80-B072-DFBD824F28CF`, isolated HOME `/tmp/oa-drag-test/home`, project `Wall Join Final` (cross state): `Wall 001=(0,0)→(4,0)`, `0.3 m`; `Wall 002=(2,2)→(2,-2)`, `1.0 m`. With CUA unable to produce a press→multi-frame move→release sequence, the drag was delivered as real CGEvent mouse events: press, 50 `leftMouseDragged` frames (~20 ms apart), release from `(0,0)` to the `x=2` baseline. During the drag the target baseline, circular snap marker and `捕捉：墙参考线` were visible (`/tmp/oa-drag-test/screen13-drag.png`); release stored `(2,0)`, reported `墙端点已连接到参考线` and trimmed the branch at the thick wall's face `x=2.5` (`/tmp/oa-drag-test/screen14-zh-result.png`). English repeat: `Wall endpoint joined at reference line` (`/tmp/oa-drag-test/screen6.png`). One Undo restored the cross and enabled Redo (`/tmp/oa-drag-test/screen7.png`); after Redo the 3D view showed no buried end cap (`/tmp/oa-drag-test/screen8.png`). |
| Endpoint handoff (automated RawInput) | pass: `wall_endpoint_drag_snaps_to_another_reference_line_and_joins`, `wall_endpoint_drag_onto_a_foreign_material_wall_body_snaps_and_joins`, `wall_endpoint_release_inside_snap_radius_joins_without_pointer_move`, and `wall_endpoint_reference_snap_wins_over_a_coarse_grid_snap`. The last case proves a 1 m grid cannot pull the pointer from target baseline `x=2.4` to `x=2.0`. Tests assert the target wall's stored baseline is unchanged and 3D hides the buried end cap. |
| Derived T trim in Plan and 3D (supplementary reloaded-model check) | pass: persisted project `Wall Join Final` with horizontal `(0,0)→(4,0)`, `0.3 m` and vertical `(2,2)→(2,-2)`, `1.0 m`; with the horizontal start stored as `(2,0)`. On reopen, Plan trims at the thick wall's face (`/tmp/openarchi-final-plan-t.jpg`) and 3D has no buried end cap (`/tmp/openarchi-final-3d-t.jpg`). |
| Same-material T join split across renovation groups (desktop, bundle `D0FD5583-C853-3379-9E3F-50329DF4C803`) | **pass**: isolated HOME `/tmp/oa-join-final.AFg79r/home`; recovered project `Same Material Join` contains `Wall 001=(0,0)→(4,0)`, `Plaster`/`Existing`/`0.2 m` and `Wall 002=(2,0)→(2,2)`, `Plaster`/`New`/`0.2 m`. Screenshot `/tmp/join-recovered.png` (3024×1976 plan viewport) shows one continuous horizontal fill across the junction. Pixel scan: the through wall's top outline at rows 1318–1322 is interrupted between x=1368–1395, exactly the branch body; both branch side outlines stop at row 1322 instead of continuing into the through wall; the fill row y=1330 is `(211,211,211)` from x=1330 through 1439, with no interior dark stroke. |

The right-drag path remains a pan and is covered by
`right_drag_still_pans_without_finishing_the_wall_draft`. Earlier CUA
right-click attempts on a two-node chain failed while right-click on a Single
draft succeeded; the same gesture then succeeded on the final bundle. That
history is an input-delivery limitation of the automation, not a code path
difference: the winit/egui event is `Response::clicked_by(Secondary)` and the
unit regression drives the same event.

## Derived geometry contract

A wall is stored as:

- reference line `start → end`, thickness `t`, per-wall `WallSettings`;
- `body_offset(t) = reference_line.preset_offset(t) + reference_offset`, where
  `Center = 0`, `Inside = -t/2`, `Outside = +t/2`.

The body occupies the lateral interval `[body_offset - t/2, body_offset + t/2]`
measured left of the `start → end` direction, before any join.

At a connected end the square end edge is replaced by the two corner points
that the adjoining wall imposes:

1. The end is connected when its reference point is within
   `(t_own + t_other) / 2 + 0.02 m` of the other wall's reference segment and
   projects inside that segment extended by the same tolerance. This covers
   drawing to a wall face as well as endpoint-to-endpoint and T connections.
2. **Coincident corner** (the other wall also ends within `1 mm` of this
   point): the corner points are the intersections of the *same side* lines.
   Both walls are oriented towards the joint, so the left side of one meets the
   left side of the other and the right meets the right. This produces the
   mitre for any angle.
3. **Offset corner** (the other wall also ends here but the reference points
   are apart, for example a return snapped to the host's face): both own side
   lines are cut against the far face of the other wall in the outward
   direction. Both corner points therefore lie on one trim plane, the outside
   corner is filled through the host's depth instead of leaving a diagonal
   bevel, and the two bodies still share an internal seam.
4. **T connection** (the other wall passes through): both own side lines are
   cut at the other wall's nearer face, so an abutting wall stops at the face.
5. Across several connected walls at one end the farthest valid corner along
   the outward direction wins, so a wall reaches past every wall it meets.
6. Parallel walls, missing intersections, or a corner further than
   `8 × (t_own + t_other)` from the end leave the square end unchanged.
7. A result that is non-convex or non-positive-area falls back to the raw
   body, so a short wall can never turn inside out.

The stored `Shape::Wall` start/end are never rewritten. Joins are recomputed
from the project on every display pass.

An endpoint drag can also land on a neighbouring wall's reference line. The
editor accepts the nearest visible same-story straight wall when the pointer is
within `12 px` of its reference segment **or inside / within `12 px` of its
body**. The reference-line test remains independent of the body test, so a
numeric reference offset that shifts the body away cannot disable baseline
capture. Dragging onto the visible face or interior of a thick wall also
selects that wall even when the pointer is farther than `12 px` from its
baseline. The pointer is projected perpendicularly onto the reference segment
and that exact point is stored as the edited endpoint on release, leaving the
rest to the same join pass. A T branch is therefore trimmed at the through
wall's near face and an L connection is mitred, in Plan and in 3D, without a
separate "join" object or stored trim polygon. Whole-line moves deliberately do
not snap.

Consequences that the previous centre-line-extension heuristic got wrong:

- angled corners no longer use a half-thickness extension, so the mitre length
  follows the corner angle (`(t/2)/tan(θ/2)` for a symmetric corner);
- walls with different thickness meet on the correct pair of faces;
- `Inside`/`Outside` reference lines and numeric reference offsets move the
  body without moving the join points onto the reference line;
- a T branch is cut at the through wall's face instead of poking through it;
- collinear walls keep square ends and their shared end edge is still removed
  from the outline.

### Interaction contract

| Gesture | Behavior |
|---|---|
| Right-click during Single/Rectangular with only the first point | Cancels the draft; no model change |
| Right-click during Continuous with at least two nodes, or Polygonal with at least three nodes | Commits every entered segment in one undo transaction and ends the draft |
| Right-click below that minimum | Cancels the draft; no model change |
| Click the start point again (Continuous with ≥ 2 nodes, Polygonal with ≥ 3 nodes) | Continuous commits after one segment with two nodes, or adds the closing segment with three or more; Polygonal closes the ring and commits |
| Double-click, Enter, Backspace, Escape | Unchanged |

Right-drag still pans the plan: finishing is wired to
`Response::clicked_by(Secondary)`, which egui only raises for a press/release
without a drag.

## Reversible choices and exclusions

- AC29 selects between several junction cases and applies connection data; this
  implementation uses the geometric side-line mitre described above, which
  reproduces the visible outline for straight walls but is not an exact
  reimplementation of `SetWall2PointAndInd`.
- Curved (arc) walls, composite skin separation, wall-panel/profile bodies,
  crossings with columns, and symbol-level junction priorities are out of
  scope.
- Wall joins are independent of material and renovation status. Plan fills,
  pens and 3D surfaces may still be grouped by material, but the footprint and
  end-cap decisions are computed once from every visible wall on the story.
  Plan outlines are also resolved from the full story perimeter first and only
  then split into pen-style paths, so same-material walls with different
  renovation states cannot stroke a buried end face. Joins remain limited to
  the same story.
- 3D end-cap hiding requires equal wall heights. Walls with different heights
  still receive the planar trim, but both caps are retained because a single
  boolean cannot hide only the buried part of a taller or shorter cap.

## Acceptance checklist

1. L corner with equal thickness: one closed outline path, square outer and
   inner corners, no diagonal bevel, no end cap.
2. L corner at 45°: the mitre follows the angle; no half-thickness spike and no
   notch.
3. L corner with different thickness: the outline uses the thick wall's faces.
4. T branch: the branch stops at the through wall's face; the through wall's
   side is not extended past its own ends.
5. X junction of four walls: the centre is filled and the four arms keep
   square ends outside the crossing.
6. `Center`, `Inside` and `Outside` reference lines and a numeric reference
   offset all join correctly, and the stored `start`/`end` are unchanged.
7. Two collinear walls: shared end edge absent, both ends square.
8. The same footprint drives plan fill, plan stroke, 3D and Trace.
9. Right-click, start-point click, double-click, Enter, Backspace and Escape
   behave as in the interaction table, with one undo entry per commit.
10. English and Simplified Chinese status text both render.
11. A concrete branch dragged into the body of a plaster wall joins and trims,
    including when the two walls have different renovation statuses.
12. A pointer inside a thick target wall body, more than `12 px` from its
    reference line, still captures that baseline.
13. A numeric reference offset that moves the target body away from its
    baseline does not prevent a pointer near the baseline from capturing it.
14. With a coarse grid active, the raw pointer still captures a nearer wall
    baseline; grid snapping may only constrain the along-wall position and
    cannot pull the endpoint away from that baseline.
15. Two same-material walls in different renovation groups meet without an
    internal plan outline; their fills remain continuous and the outer
    perimeter is still stroked with each wall's own pen style.
