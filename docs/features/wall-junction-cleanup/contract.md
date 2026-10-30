# Palette three-state feedback and multi-wall junction cleanup — 2026-10-29

## User outcome

Wall pet palette must show three simultaneous states: current active command strongest/darkest, hover candidate intermediate, normal unchanged. Hover browsing must not reset the active command's point stages/numeric values. Clicking a candidate or moving back to the canvas activates it; this preserves the prior no-extra-click workflow while giving the requested distinct states. Held canvas drags can cross the palette and return/release to commit the new command once.

At three or more finite straight-wall reference endpoints meeting at one point, derive a clean junction without pairwise-extension spikes or a central hole. Model endpoints, dimensions, properties, undo and persistence remain unchanged by display cleanup. Plan fill/outline/selection, Trace and ordinary selected/unselected 3D must agree. Existing two-wall L joins, branch-to-through T joins and layer intersection-group semantics must retain their regressions.

## Evidence and scope

- User requirement is the authority for palette states and absence of unintended projections.
- Readable source: `geometry::joined_end_corners` currently chooses the farthest outward corner independently across every wall pair. That rule is not a bounded multi-way junction calculation.
- Official AC29 help: https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-272.htm — wall reference lines must intersect, material priority controls cutting, and Junction Order determines which two walls meet before others. This does not reveal proprietary implementation or precise tie-breaking.
- Runtime API observation: AC29 INT build 5101, Tapir 1.7.1, isolated copied project `target/ac29-junction-probe.pln`, PID 35645, API port 19724. Original user project/PID 51530/port 19723 kept untouched. Exact requests/results are `archicad-cases.json`, `archicad-details.json`, `archicad-extra.json`. Main executable SHA-256 d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505.
- Five initial equal-material/equal-priority centered test groups: Y120, T, deformed T from the preceding user workflow, four-ray fan, X. Additional unequal-width Y, narrow three-ray fan and Inside-reference Y. API floorPlanPolygons are generated display polygons, not a recovered internal algorithm. Compare their combined exposed perimeter/coverage, not internal polygon ownership or triangulation.
- Native AX Zoom to Selection plus screenshot `archicad-moved-t.png` visually confirms the deformed three-wall junction. REA native capture returned window-mismatch on the recovery window; no Evidence ID invented. Direct API data and native screenshot/AX are the runtime evidence for this pass.

## Acceptance

Palette: simultaneous active/hover/normal drawing, candidate browsing/commit, keyboard cycling, numeric/stage preservation, no accidental model commit.
Geometry: captured AC scenarios compared to measured polygons; no self-intersection, central gap or overextended corner; deterministic input ordering, start/end reversal and rotation; 0/different layer groups stay separate; existing two-wall/T/outline/renovation tests stay green. Native final package plan/3D comparison in both languages; required checks, fresh app UUID/signature and explicit remaining limits in validation.

## Reconstruction decision

Measured exposed union areas (m²; 4 m walls, 0.3 m width): Y120 AC 3.561028858 vs angular-neighbor caps 3.561028857; T 3.555 and X 4.710 agree; deformed T AC 3.424127964 vs angular caps 3.424840430; Fan4 AC 4.693368673 vs angular caps 4.627269825. AC's Fan4 itself exposes a backward first-pair miter at (-0.850692,-0.15) relative to the shared node. Thus matching all AC ordering-dependent cuts would contradict the user's requested removal of protrusions.

OpenArchi chooses angular-neighbor face intersections plus a center cap for three or more shared endpoints. This is original geometry, not recovered proprietary source. It matches the measured symmetric Y/T/X perimeter and deliberately avoids the non-neighbor backward spike in the fan. Arbitrary priority/sequence equivalence, composite walls and curved-wall junctions remain outside this slice. Different widths, reference sides, short walls and degeneracies require bounded caps and regression coverage. Derived cleanup does not modify persistent model points or add undo entries.

## Experience candidates (pending developer confirmation)

- A multi-way junction cannot be repaired by independently minimizing or maximizing four miter corners: it needs sector ownership and explicit center coverage; test both protrusions and holes.
- Distinguish active command from hover candidate in state as well as paint. Browsing icons must not reset a staged operation until the candidate is actually adopted.
- When a footprint stops being convex, update every consumer together: plan/selection/Trace fills, outline clipping, selected and unselected 3D. A convex painter can silently fill a geometric notch.

These are proposals only; no reusable AGENTS.md entries were changed by this follow-up.
