# Polyline drawing

## Evidence and scope

- Reference: Archicad 29, ARM64 macOS, running Untitled project, window 38986.
- Runtime observation `ev_3f099f7142777dcb228abf6f1dc8851fffbd1b77f47a44b7655570a169225428` shows the Polyline tool active and the status `Enter First Node of Polyline.`
- The visible Info Box shows the default layer `2D Drafting - General`, line type `Solid`, pen `62`, line category `Drafting Line`, and three Geometry Method controls. Only their visible presence and the first control's selected state were observed; their behavior was not tested.
- Kimi Code's session `e2fb85ba-1093-45b8-abae-c17d8c52bc6c` records its Polyline task and confirms that its turn failed during toolbox coordinate interaction, before implementation.
- Runtime evidence does not yet establish finish, close, cancel, snapping, or 3D behavior. Those remain unknown for Archicad.
- User-provided workflow observation for this implementation: select Polyline and the cursor is crosshair; press/release at the start node transitions from solid pencil to hollow pencil; each subsequent segment follows the cursor and displays distance/angle; X/Y, 30°/60° and nearby point/perpendicular alignments add a line cue; left click commits a node, right click finishes, and clicking the start point closes and finishes. This is the user's observed behavior, not an independently captured REA replay.

## OpenArchi workflow contract

- Goal: draw an editable 2D polyline annotation on the current story, as either an open path or a path closed by clicking its start node.
- Entry: select Polyline in the Document toolbox group or press `P`; no element selection is required.
- Interaction: the first left click starts a chain. After release, a hollow pencil follows the pointer. Left click commits each next node; right click commits the open chain. Clicking the first node closes and commits the chain. Escape cancels. Enter finishes when Tracker is not locked; when Tracker is locked, Enter adds the numeric point and continues.
- Defaults and numeric input: current story; distance in metres and angle in degrees follow the pointer while unlocked. Tracker edits lock values until the next node is committed or Lock is cleared. With snapping enabled (default), endpoint/midpoint and 100 mm grid snaps remain active; 30° direction increments, horizontal/vertical alignment, nearby node alignment, and perpendicular direction cues are accepted within 9 screen points.
- Cursor states: crosshair before the first point; solid pencil while primary is held to place a point; hollow pencil after a point is committed; hollow pencil with a line cue at a resolved geometric constraint.
- Defaults: dark line tone derived from the active material; no fill or extrusion. The Archicad Info Box defaults above are recorded as reference observations and are not represented as implemented OpenArchi settings.
- Effects: show the same path in plan, select the created element, include it in project JSON, and make creation one undo/redo transaction. A selected path is highlighted green, with black filled circles at its editable nodes. It is 2D-only and does not appear in 3D.
- Invalid input: ignore consecutive nodes closer than 10 mm; reject a finish with fewer than two distinct nodes while retaining the active chain; discard in-progress nodes on Escape. Clicking within 9 screen points of the start closes and commits a chain with at least two existing nodes by appending the first node.
- Exclusions for this slice: filled area semantics, rectangle/rotated-rectangle geometry methods, arc segments, line category/pen controls, and 3D rendering.

## Acceptance checklist

1. Press `P`; the status prompts for the first Polyline node.
2. Click a start point; after release, verify the hollow pencil, live segment, and distance/angle tracker.
3. Move near X/Y, 30°/60°, an existing node, and perpendicular to a nearby segment; verify the alignment line cue and tracker values update.
4. Edit distance or angle, confirm the target locks, then left click to add a node. Right click finishes an open path.
5. Draw at least three nodes and click the first node; the path closes and commits. Undo removes it; redo restores it. JSON round-trip preserves the closing segment.
6. Select a completed path; verify the line turns highlighter green and its nodes show black filled circles. Escape cancels an unfinished path without changing the project. The polyline remains selectable/deletable in plan and is omitted from 3D.

Reference runtime replay of the node/finish/cancel sequence remains unverified; the interaction above is the OpenArchi feature contract, not a claim of exact Archicad parity.
