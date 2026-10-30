# Move a selected wall junction — 2026-10-29

Authority: explicit user requirement; readable source `selected_wall_baseline_hit` previously returns only one `(wall, endpoint)` and `apply_wall_drag_preview` mutates only that wall. No new AC29 runtime observation or equivalence claim.

Start: Select tool, two or more selected straight walls with a shared reference-line point. Prioritize a true geometric junction within 10 screen points over any single wall endpoint/edge. Geometric membership uses a small world tolerance (0.0001 m), distinct from pointer hit tolerance. Collect all selected visible same-story straight walls passing through that finite point; unrelated or unselected walls stay unchanged. At a projected 3D reference-edge junction, require matching story and edge elevation.

Click starts the shared Move node preview. Press-drag directly also starts it. All participating endpoints move to exactly one target; remote endpoints remain fixed. At T/X interior crossings, insert a split in each through-wall, following the existing atomic InsertWallNode semantics, metadata preservation and opening-host remapping. This is an explicit OpenArchi modeling choice, not established Archicad equivalence. Coincident endpoints do not create new walls. Original model stays unchanged during preview; drag release or numeric Enter commits once. Escape drops the entire draft. Palette hover switches pending commands; returning to the first icon resumes shared-node semantics. Preserve the user's original selection plus any newly created pieces.

Use the existing compact palette/tracker, with first command labelled Move node for a junction. Plan and 3D derive from the same result. Typed `MoveWallJunction` builds/validates a private candidate before replacing anything. Locks/groups, invalid/short segments, hosted openings and unsupported curved/profiled walls must never leave partial changes. Exact model JSON round trip, one undo/redo and rejection atomicity required. Existing single-wall endpoint behavior stays available when only one wall participates.

Exclusions: curved/profiled-wall junction deformation and arbitrary screen-space overlaps without a shared world intersection. Different-elevation projected crossings are not a physical joint. No new automatic wall intersection/constraint graph persisted.

Acceptance: two endpoints, three/four endpoints, rotated view, T/X interiors, nearby unrelated endpoints, unselected neighbor, locked participant, invalid destination, hosted opening handling; private preview, release/Enter/cancel, one undo/redo and JSON; selected 3D junction does not orbit; native final packaged app in both languages.

## Implementation evidence and candidate lessons

Readable source: `reshape::wall_junctions` derives finite selected-wall intersections; `ReshapeCommand::MoveWallJunction` validates a private project, updates every participating endpoint or uses existing InsertWallNode for through-walls; `start_wall_junction` and plan/model hit paths prioritize shared nodes. No persisted constraint graph or new project schema. The existing `Move node` translations in both locale files are reused.

Proposed reusable lessons, pending developer confirmation (AGENTS.md unchanged):

- Gather every geometric participant before starting a junction edit; picking one topmost element is insufficient for a multi-selection node gesture. Keep geometric coincidence tolerance separate from screen-space pointer tolerance.
- Build and validate the entire multi-element candidate before committing; one locked participant or invalid opening must reject the whole edit without partial movement or history entries.
- egui Area sizing passes can use invisible temporary rectangles. Suppress custom hover activation and canvas exclusion during that pass, especially for sessions created on the first drag frame.
