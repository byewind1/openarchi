# Door and Window Openings

## Goal

Place a rectangular Door or Window into a straight wall, see the opening in
plan and in 3D, select it, edit its dimensions, move it along the wall, undo the
change, and save/reopen it through the JSON project adapter.

## Interaction contract

- Toolbox `Design > Door` and `Design > Window` activate the placement tool.
- A click on a straight wall on the current story creates one opening.
- The opening is centred on the clicked point, projected onto the wall reference
  line. The centre is clamped so the full width stays inside the wall.
- Clicking outside a wall reports `Click on a wall to place a door/window`.
- Door defaults: width 0.90 m, height 2.10 m, sill 0.00 m.
- Window defaults: width 1.20 m, height 1.40 m, sill 0.90 m.
- Select tool: clicking the opening symbol selects the opening, not the host wall.
- Dragging a selected opening moves it along its host wall; the component
  perpendicular to the wall is ignored and the centre offset is clamped so the
  full width stays inside the wall.
- Dragging a selected wall moves its openings with it because `offset` is
  measured from the host wall start.
- Arrow-key / nudge moves follow the same rule.
- Delete on a wall cascades to its openings as one undoable change.
- Info Box edits width, height, sill and offset; invalid values are rejected
  without changing the model.
- Undo/redo restores the opening state exactly, including cascade deletes.

## Geometry contract

- `Shape::Opening { host_wall, kind, width, height, sill, offset }`.
- `offset` is the distance, in metres, from the host wall reference-line start
  to the opening centre.
- `sill` is measured upward from the host story elevation.
- The opening belongs to the same story as its host wall.
- Validation rejects: missing host, host that is not a wall, different story,
  non-positive dimensions, sill below the host wall's bottom offset, opening
  wider than the host wall, an opening that extends past either wall end, an
  opening whose top passes the host wall top, or two openings whose intervals
  overlap on the same wall.
- Plan: the wall is painted first; the opening erases the wall band with the
  plan background colour and draws a door leaf/arc or window glazing lines.
- 3D: rectangular holes only, for straight walls. The wall is emitted as solid
  piers plus sill and lintel blocks. Doors stay open; windows receive one thin
  glazing pane. Walls with openings are excluded from the joined-wall outline
  optimisation.

## Explicit limits for this slice

- No curved walls, round or arched openings, frames, sashes or door leaves in 3D.
- No multi-wall cutting, no openings crossing wall junctions or story boundaries.
- Copy/paste of an opening works only when its host wall is copied in the same
  operation; a standalone opening paste is skipped.
- Mirror/rotate operate on host geometry; opening offsets follow the rotated
  wall reference line but are not independently transformed.
- Marquee 3D clipping of a wall with openings still displays the clipped wall as
  solid; plan and unclipped 3D show the holes.

## Acceptance checklist

1. Place a door on a 4 m wall; plan shows the gap and leaf, 3D shows the hole.
2. Select the door and set width 1.10, height 2.00, sill 0.05, offset 2.00;
   plan and 3D update together.
3. Drag the door along the wall; it cannot leave the wall.
4. Move the wall; the door keeps its offset and moves with it.
5. Delete the wall; the door disappears in the same undo step; undo restores both.
6. Save JSON, reopen it; the door/window and dimensions round-trip.
7. Switch English and 简体中文; Door, Window, Width, Sill Height and Host Wall
   labels follow the active language.

## Verification status

- `./scripts/cargo.sh test --locked --test openings`: 10 tests pass, covering
  placement defaults, end clamping, host/story/size/overlap and vertical
  offsets, drag clamping, cascade delete and undo, wall copy with remapped
  hosts, standalone-opening paste rejection, plan hit testing and JSON
  round-trip.
- `./scripts/cargo.sh test --locked --all-targets`: 223 tests pass, with the
  manual performance baseline ignored.
- `python3 scripts/check-localization.py`: 456 literal keys, no missing,
  duplicate or untranslated entries.
- Packaged `dist/OpenArchi.app` `A5BCB793-F9D4-3652-A4C4-076E3CFBDFC5` was
  launched on macOS: first frame was Simplified Chinese, the demo showed 12
  elements, Undo/Redo were disabled, the toolbox showed 门/窗, and the Door
  Info Box showed 宽度 / 高度 / 窗台高度.
- Not yet manually verified on this package: placing a door/window with the
  desktop pointer, the 3D hole appearance, English rendering of the new labels,
  and save/reopen through the native dialogs. Automated tests and static code
  cover these paths, but they are not desktop evidence.
