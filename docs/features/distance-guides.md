# Distance Guides geometry contract

Scope: current-story straight walls in OpenArchi plan view. Distances use metres.
This is an original implementation; interaction matching is documented by the
parent Guides investigation. No Archicad runtime evidence is asserted here.

## Reversible implementation assumptions

- A point being placed queries nearby walls. A selected wall queries other walls
  from its midpoint for lateral measurements and its facing endpoint for axial
  measurements. At most the nearest projected measurement in each of four
  cardinal direction buckets is returned. Projection must lie on the context
  wall segment; no infinite extensions are silently used.
- Reference-line mode measures wall axes. Edge mode measures facing physical
  sides, including the selected wall's half thickness for lateral measurements.
  Overlapping/touching references do not produce positive distance labels.
- Editing a lateral distance translates the selected wall. Editing an axial
  distance stretches the facing endpoint while keeping the opposite endpoint
  fixed. Oblique context walls use perpendicular translation. Distances are
  finite, positive, below 100000 m, and every result must pass Project::validate.
- Numeric point placement preserves the context side and perpendicular direction.
  Context/selection changes invalidate an old measurement; callers recompute it.
- Geometry functions return a new project without mutating input. The Editor
  owns undo/redo and confirmation. Cancellation discards the proposed project.
  Committed wall geometry uses existing project serialization and shared views.
- Doors, windows and other element kinds are excluded: their dedicated distance
  semantics are not implemented or claimed by this module.

Evidence: **readable source** in `src/model.rs` (`Point`, `Shape::Wall`,
`Project::validate`) establishes centered straight wall geometry, coordinates and
validation limits. All measurement and edit rules above are **inference / product
assumption**, to be checked against the parent feature's evidence and UI contract.

## Acceptance checklist

1. Four room walls return four perpendicular point distances on the active story;
   excluded IDs and walls outside their finite projected segments are ignored.
2. Physical edge distance subtracts half the context wall thickness; selecting a
   parallel wall subtracts both facing half thicknesses.
3. A 2 m lateral distance edited to 3 m moves the entire selected wall by 1 m away;
   an axial distance change moves only its facing endpoint.
4. Rotated walls retain perpendicular projections. NaN, infinity, zero, negative
   input, invalid wall collapse, and stale measurements fail without mutation.
5. Edited geometry passes validation and JSON round-trip. Parent UI integration
   verifies one undo step, redo, cancel, visibility and packaged-app behavior.
