# Wall selection reference-line highlight — Archicad 29 / OpenArchi

## Goal

When one or more walls are selected in Plan, show the wall reference line
(baseline) as clearly as Archicad 29 does. The treatment must be reusable by
other plan surfaces and must not change wall geometry, hit-testing, selection,
undo or persistence.

## Evidence

| Claim | Type | Source / evidence | Limits |
|---|---|---|---|
| A selected wall uses a pale green fill, a dark green outline, a solid pure-blue reference line and black square handles at both reference-line ends | runtime observation | CUA capture of Archicad 29 `29.0.0 v1 INT (5101)`, SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`, 2026-10-18; sample capture `/tmp/ac29-wall-selected2.png` | No REA Evidence ID. Screenshot sampling only; the live Archicad project was not modified. |
| Sampled fill color is RGB `195, 213, 155`; the blue line is approximately RGB `0, 0, 245`; the outline is a dark green close to RGB `0, 102, 0` | runtime observation | Pixel sampling of the same capture | Anti-aliasing makes the outline sample approximate; exact AC29 pen-table values are unknown. |
| The blue line follows the configured reference-line location, not necessarily the visual wall centre | runtime observation | Same capture; AC29 Info Box showed `Reference Line Location: Core Outside` | Only one reference-line location was captured. |
| An unselected wall shows neither the blue line nor the endpoint handles | runtime observation | `/tmp/ac29-wall-unselected.png` | Same CUA limitations. |

OpenArchi draws its own original egui shapes using the observed colors and
proportions. No Archicad source, resource or asset is copied.

## Behavior contract

- Selecting a wall in Plan paints its footprint pale green and outlines it dark
  green. The wall's `start`–`end` reference-line anchors are then painted as a
  solid blue line, with a black square centred on each anchor.
- `WallSettings::geometry.reference_line` selects the body side: `Center`
  centres the body on the line, while `Inside`/`Outside` shift it by half the
  wall thickness to that side. `geometry.reference_offset` is then added on
  top. Neither setting moves the highlighted line itself, matching the
  observed Archicad 29 behavior.
- Every wall in a multi-selection receives the treatment. All selected fills
  are painted first and all baselines second, so adjacent fills cannot hide a
  neighbour's reference line.
- The existing wall-length label remains, using the dark-green selection
  colour. It is an OpenArchi aid, not a claim about AC29's selected-wall label.
- Selection hit-testing, model geometry, project JSON, undo/redo and 3D output
  are unchanged.

## Reuse

- `geometry::wall_reference_line(&Element) -> Option<(Point, Point)>` is the
  single model-level source for a wall baseline.
- `paint_selected_wall_baseline(&Painter, Pos2, Pos2)` owns the blue stroke and
  black handles. Another plan, preview or test can call it with screen points.
- Colours and dimensions are named constants in `src/app.rs`:
  `WALL_SELECTION_FILL`, `WALL_SELECTION_OUTLINE`, `WALL_REFERENCE_LINE`,
  `WALL_REFERENCE_LINE_WIDTH` and `WALL_REFERENCE_HANDLE_SIZE`.

## Acceptance checklist

1. With a wall unselected, Plan shows no pure-blue baseline and no black
   wall-end handles.
2. Select a wall: its fill/outline change, a blue line connects both reference
   anchors and a black square is centred at each anchor.
3. Set a non-zero reference offset: the wall body moves, the blue line remains
   on the original reference-line anchors.
4. Select several walls: each visible selected wall keeps its own baseline.
5. View Plan and 3D: no model, selection-count, undo or dirty-state change is
   caused by the highlight itself.

## Automated coverage

- `geometry::marquee_tests::wall_reference_line_stays_put_when_the_body_offset_changes`
  checks the baseline anchors and the shifted footprint.
- `geometry::marquee_tests::wall_inside_and_outside_locations_move_the_body_not_the_line`
  checks that `Inside`/`Outside` move the body by half the thickness in
  opposite directions while the baseline anchors stay fixed.
- `app::ui_tests::selected_wall_draws_the_archicad_style_reference_line_and_handles`
  renders the Plan, proves the baseline appears only for the selected wall, and
  checks the stroke colour/width, black 6 px handles and their endpoint match.

## Remaining unknowns

- The capture covered one AC29 reference-line location and one display scale;
  hover, locked, grouped and multi-selection variants were not compared.
- Exact AC29 pen definitions, highlight opacity and high-DPI sub-pixel
  behaviour are not claimed.
- CUA screenshots have no REA Evidence ID.
