# Wall geometry methods — Archicad 29 / OpenArchi

## Goal

The four Geometry Method choices in the Wall tool Info Box must drive four
different drawing interactions. They are not labels copied onto the same
two-click rectangle workflow.

Target: Archicad 29 `29.0.0 v1 INT (5101)`, SHA-256
`d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`.
OpenArchi uses metres and world-axis plan coordinates.

## Evidence

| Claim | Type | Source | Limits |
|---|---|---|---|
| Single Straight Wall creates one wall at a time from two clicked endpoints | public documentation | Graphisoft AC29 help, [Create a Straight Wall](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-8.htm) | Documentation only; this pass did not capture the AC29 interaction with REA. |
| Chained Wall places successive segments, completes with a double-click, Backspace removes the last input segment, and Cancel/Delete removes the whole chain | public documentation | Graphisoft AC29 help, [Create a Chain of Walls](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-10.htm) | The help also describes curved segment choices in the pet palette; those are outside this slice. |
| Rectangle Wall is defined by a diagonal and its four sides remain orthogonal to the Normal Grid, unaffected by a Rotated Grid | public documentation | Graphisoft AC29 help, [Create a Rectangle of Walls](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-11.htm) | The separate Rotated Rectangle Wall method is not one of the four OpenArchi choices. |
| Polygon Wall draws a free contour, may contain straight and curved edges, has one reference edge, warns about self-intersection, and uses only simple fills | public documentation | Graphisoft AC29 help, [Create a Polygon Wall](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-13.htm) | OpenArchi currently emits a ring of ordinary wall segments, not a single PolyWall element. |

No Archicad source, private resource or branded asset was copied. The input
state machines and controls are original OpenArchi code.

## Behavior contract

All methods share the current layer, thickness, height, reference-line location,
reference offset and remaining `WallSettings`. A completed method commits one
undo transaction.

| Method | Input sequence | Result |
|---|---|---|
| Single | Click the start point, then click the end point. Tracker Enter uses the locked length/angle or current cursor. | One `Shape::Wall` segment. |
| Continuous | Click each node in turn. Right-click with at least two nodes, double-click or press Enter without a locked Tracker to finish. Clicking the first node again finishes a two-node wall, or closes the chain with a closing segment when three or more nodes exist. With a locked Tracker, Enter adds the computed node; a separate **Finish** button completes. Backspace removes the last input node. Delete or Escape cancels the whole draft. | Adjacent wall segments sharing each input node; a start-point click also adds the closing segment. The last created segment remains selected so the single-wall Info Box stays usable. |
| Rectangular | Click one corner, then the diagonally opposite corner. | Four wall segments whose sides follow the world X/Y axes. Ortho and Shift do not collapse the rectangle to a line, and the Tracker uses ΔX / ΔY. |
| Polygonal | Click each contour node. Right-click with at least three nodes, click the first node, press Enter, or double-click to close and commit. | A closed ring of wall segments. The ring is normalized counter-clockwise so `Inside` consistently means the enclosed side; all segments commit together. |

Shared behavior:

- The plan preview shows the actual method result: one segment, the current
  chain, four rectangle sides, or the closed polygon ring.
- Changing the method in the Wall-tool Info Box cancels an unfinished draft and
  resets the status hint before the next click. This prevents vertices from one
  state machine leaking into another.
- A method still needs valid geometry: walls must be at least 10 mm, a
  rectangle needs non-zero width and depth, and a polygon needs at least three
  non-collinear nodes with non-zero area.
- Escape, Delete and the Tracker **Cancel** action discard the draft without an
  undo entry. Finishing creates exactly one undo entry for the whole result.
- Right-click is a finish gesture only when the draft has enough input
  (Continuous: two nodes, Polygonal: three). Otherwise it cancels without a
  model change. Right-drag continues to pan and does not finish the draft.

### Reversible product choices

- AC29 describes Polygon Wall as a two-step contour-and-complete workflow with
  a hammer cursor. OpenArchi commits the ring as soon as it is closed. This
  keeps the first slice simple, but it is not a claim of identical AC29 timing.
- Continuous accepts Enter as a finish shortcut in addition to the documented
  double-click. The locked-Tracker variant adds a point instead, matching the
  existing numeric-input workflow.

## Model mapping and limits

The current model has no PolyWall entity. Therefore Continuous, Rectangular and
Polygonal results are stored as multiple `Shape::Wall` elements. Plan, 3D,
selection, JSON persistence and undo reuse the existing wall path, and each
method has one undo transaction because the segment list is committed inside
`Editor::create_wall_segments`.

Explicit exclusions:

- Curved chained segments, tangential/center-point arcs and the AC29 pet
  palette segment choices.
- Rotated Rectangle Wall. Only the four Info Box choices above are implemented.
- A single editable Polygon Wall element, its movable reference edge,
  self-intersection warning, polygon-specific surface assignment and
  simple-fill/composite restrictions.
- Multi-selection of a whole chain or polygon ring as one logical group.
- Editing an existing wall's geometry method does not convert its geometry; the
  method currently controls creation of the next wall.

## Automated coverage

- `tests/workflows.rs`
  - `continuous_wall_method_builds_a_connected_chain_in_one_undo`
  - `continuous_wall_clicking_the_start_closes_and_commits_the_chain`
  - `continuous_wall_backspace_removes_only_the_last_input_node`
  - `rectangular_wall_method_builds_four_world_aligned_segments`
  - `polygonal_wall_method_closes_a_counter_clockwise_ring`
  - `wall_geometry_methods_are_not_just_labels`
- `src/app.rs` `ui_tests`
  - `wall_tool_info_box_geometry_method_drives_real_drawing_behavior`
  - `continuous_wall_double_click_finishes_the_chain`
  - `continuous_wall_clicking_the_start_point_closes_the_chain`
  - `right_click_finishes_a_continuous_wall_draft`
  - `right_click_cancels_single_and_rectangular_drafts_without_an_undo_entry`
  - `right_drag_still_pans_without_finishing_the_wall_draft`
  - `polygonal_wall_double_click_on_start_closes_once`

## Acceptance checklist

1. In the Wall-tool Info Box, choose each of the four methods and draw in Plan.
   The click sequence and preview must match the table above.
2. Finish Continuous and Polygonal drafts; one Undo must remove every segment
   from that operation.
3. Press Backspace during a chain; only the last input node disappears. Press
   Delete or Escape; the entire draft disappears without a model change.
4. Change the method in the Info Box while a draft is open; the draft must be
   cleared and the new method must start from its own first click.
5. Check that Rectangle sides remain world-axis aligned while Ortho is off and
   on, and that the Tracker shows ΔX / ΔY for that method.
6. Verify the status bar and Info Box in English and Simplified Chinese.

## Desktop validation

The four method state machines were exercised on packaged builds in
Simplified Chinese with project `Courtyard House` and 12 existing elements; no
project was saved.

| Method | Interaction performed | Observed result | Undo |
|---|---|---|---|
| Single | Clicked a start point, then a Tracker length of `4.000 m` and Enter | Status `已创建构件 016`; one new wall | One Undo removed it |
| Continuous | Added four nodes with the Tracker; finished the chain | Status `已创建 4 段连续墙`; elements 12 → 16 | One Undo restored 12 |
| Rectangular | Clicked one corner, then the diagonally opposite corner | Status `已创建 4 段矩形墙`; elements 19 → 23 | Undo removed the four sides |
| Polygonal | Added Tracker nodes and closed the contour | Status `已创建 7 段多边形墙`; elements 12 → 19 | One Undo restored 12 |

On the `49906133` bundle the two-node Continuous gesture was rechecked:
right-click finished 12 → 13 with `Created 1 connected wall segments`; the
Chinese hint read `墙 · 连续 · 逐点点击 · 右键、双击或回车完成` and the English
hint `Wall · Continuous · Click nodes · right-click, double-click or Enter to
finish`. Clicking the start point and pressing Enter had been verified on the
immediately preceding `D030826E` bundle with identical geometry source. The
current `638273BF` package adds wall reference-line editing, endpoint snapping
and its palette handling but keeps the wall-creation state machines unchanged
from `49906133`; the Enter and click-start results are therefore not claimed as
re-run on it, while the final package's two-node right-click finish was
rechecked.

## Packaging identity

- App: `dist/OpenArchi.app`
- Release and bundle arm64 UUID: `638273BF-9D59-390D-B31F-AC6D7083F491`
- Release SHA-256: `add89061e3854341c147f352810b4ce87f659d1634b406a4ff23a2d7a5f230c7`
- Bundle tree SHA-256: `a3fd870f43c308fbeb66228eea86b4c7d3266283d563383e133b97a677f8e584`
- macOS zip SHA-256: `8700685dae580a540b0a0268ed2726a8442507091a1632d288165b98167c0b53`
- `codesign --verify --deep --strict dist/OpenArchi.app`: passed

Automated checks: `./scripts/cargo.sh fmt --all -- --check` passed;
`./scripts/cargo.sh check --locked --all-targets` passed;
`./scripts/cargo.sh test --locked --all-targets` passed **253 tests** (205
library, 7 Guides, 10 openings, 31 workflows; one manual performance test
ignored); `python3 scripts/check-localization.py` passed with 462 literal keys.
`python3 scripts/ac29-workflow.py verify --force` is recorded in
`docs/validation.md`.

Language checks on the final package: the title-bar selector switched
简体中文 → English → 简体中文. English surfaces included `Wall`, `Geometry
Method`, `Single / Continuous / Polygonal / Rectangular`, `Reference Line:
Center / Inside / Outside`, `Reference Offset` and `Thickness`; Chinese
restored `墙`, `几何方法`, `单线 / 连续 / 多边形 / 矩形`, `参考线：居中 /
内部 / 外部`, `参考线偏移` and `厚度`. The window title followed the
language. The model stayed at 13 elements after the finish check, `Undo`
stayed enabled and `Redo` stayed disabled across both switches. After choosing
简体中文 and restarting, the first frame was Chinese.

## Remaining gaps

- Continuous, Rectangular and Polygonal results are multiple `Shape::Wall`
  segments, not a single AC29 PolyWall element. There is no polygon reference
  edge, curved edge, pet palette, self-intersection warning or polygon-specific
  surface/composite behavior.
- Continuous has no AC29 curved-segment pet palette. Rotated Rectangle is not
  one of the four implemented choices.
- Editing an existing wall's Geometry Method stores the property but does not
  convert existing geometry.
- A whole chain or ring is not reselectable as one logical group after commit.
- The Archicad comparison is based on Graphisoft AC29 public help; this pass did
  not capture AC29 runtime interaction with REA, so no REA Evidence ID exists.
