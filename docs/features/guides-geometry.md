# Guides geometry contract

## Scope and evidence

Implement original, pure plan geometry for guide lines, guide circles and temporary
snap points. All coordinates and distance settings use metres. Geometry is independent
of egui; callers own hover lifetime, drawing, confirmation, undo and persistence.

Public documentation: [Archicad 29 Snap Points](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-48.htm),
read 2026-10-15 with agent-reach's Jina web reader. It documents Half, Divisions
(3–100), Percent, Distance, Best Division by Distance; nearest endpoint or midpoint
origins for Percent/Distance; and node, intersection and entire-element scopes.
Best Division minimizes difference in segment **length**, not difference in segment
count. This is documentation evidence, not captured runtime equivalence.

Readable-source evidence: `src/model.rs` (`Point`, `Shape`, `ArcSegment`, `Element`)
and `src/geometry.rs` (`arc_center_sweep`, `footprint`) establish OpenArchi's plan
model and exact circular-arc representation. Existing files are already modified
in the working tree; this slice does not replace those changes.

## Original implementation choices

- Hover chooses the closest eligible path within a world-space tolerance, on the
  current story. Wall reference paths, slab perimeter, exact polyline arcs and stair
  footprint are supported; edge extraction additionally exposes wall perimeters.
- Entire-element length follows connected edges and exact arc lengths. Closed
  perimeters begin at their stored first vertex. Nearest endpoint uses Euclidean
  cursor distance, even when a folded path has a different nearest endpoint by
  path length. A nearest-endpoint tie starts at
  that first vertex. Midpoint percent follows the half containing the cursor;
  midpoint distance repeats in both directions and includes the midpoint itself.
- Between Intersections brackets the hovered path coordinate with exact intersections
  against other same-story element paths, plus path endpoints. Coincident/overlapping
  paths have no unique isolated crossing and do not subdivide the interval.
- Segment, infinite line, circle and exact circular-arc guides validate finite, nondegenerate geometry within the local modeling bounds.
  Guide snapping includes nearest projections and isolated pair intersections.
- Distance-mode output is capped at 2,048 nearby markers to bound allocation for
  extremely small distances on large elements. The mathematical interval remains
  unchanged; the window follows the cursor. Percentage is strictly between 0 and 100;
  distance must be positive and finite.
- Pure methods do not mutate a project. Persistent-guide ownership, serialization
  and undo belong to the integration layer; settings/guide values are serde types.

## Acceptance checklist

- Half/division/percentage/distance/best-distance markers match explicit expected
  metre coordinates, including near-endpoint reversal and midpoint origin.
- Best division for length 2.45 and desired distance 1.0 chooses 3 parts
  (0.81667), rather than blindly rounding 2.45 to 2 parts (1.225).
- Exact arcs interpolate and project using circle geometry, not tessellation.
- Entire paths accumulate edge lengths; intersection scope brackets actual crossings.
- Invalid settings/geometry produce validation errors; cross-story elements and guides
  are excluded; serde round-trip preserves typed settings and geometry.
- Integration checks must cover rendering, toggles, hover lifetime, undo and reopening;
  pure geometry tests do not claim desktop verification.

## Verification performed

`./scripts/cargo.sh test --locked --lib 'guides::tests::'` passed on 2026-10-15:
16 tests in this module plus 8 matching `distance_guides` tests (24 total, zero
failures). System `cargo` is too old for edition 2024; the repository wrapper uses
the installed compatible toolchain. Formatting was attempted but the installed
stable toolchain lacks its `rustfmt` component. Desktop packaging and UI acceptance
are owned by the integrating task and are not asserted here.

Integration follow-up: rustfmt was installed successfully; guide arcs gained exact projection/intersections and signed radial/normal offset. Final checks and bundle verification are recorded in `guides.md` and `../validation.md`.
