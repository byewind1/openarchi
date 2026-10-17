# Smart cursor feedback

## Evidence and scope

Target reference: Archicad 29 INT help; local Archicad 29 is installed and was running in an Untitled project (macOS, arm64). Public documentation identifies a smart cursor whose shape varies by active tool, whether input/editing is underway, and proximity to empty space, reference lines, other edges, intersections, and nodes. It lists Arrow-tool, other-tool, during-input/editing, Magic Wand, Trim, parameter pickup/transfer, and additional cursor families. See [The Intelligent Cursor](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-39.htm). Selection dots also distinguish regular, locked, grouped, hotlink/XREF, editable-hotspot, railing, and Morph cases; see [Selection Dots](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-27.htm).

Evidence basis:

- **Public documentation:** The Graphisoft pages above define the cursor families and selection-dot cases.
- **Runtime observation unavailable:** The local Archicad 29 app is installed, but the available UI controls provide click/drag actions rather than a pointer-only hover action. The prior REA capture also could not bind a matching Accessibility window. No Archicad cursor shape is claimed as runtime-observed.
- **Readable source:** OpenArchi supports Select, Marquee, Wall, Slab, Stair, and Polyline. `geometry::plan_hover_kind` classifies plan hover as editable node, edge, element body, or empty. Wall endpoints, slab corners, and Polyline vertices are node targets; visible plan edges and Polyline segments are edge targets. Stair handles and locked/grouped states are not implemented.
- **Inference/design choice:** Use the platform's ordinary arrow over element bodies, and draw original checkmark/triple-ray cues beside that arrow over nodes/edges. This implements the user-requested simplified Arrow-tool subset; it does not reproduce proprietary cursor artwork or the full heavy/light/reference-line/intersection distinctions.

The AC29 help page's documented cursor matrix is broader than a simple hover/no-hover pair:

| Context | Empty | Reference line | Other edge | Tangent / perpendicular | Intersection | Reference-line node | Other node |
|---|---|---|---|---|---|---|---|
| Before input, Arrow | Empty-area cursor | Heavy Mercedes | Light Mercedes | Tangent / perpendicular variants | Intersection | Heavy check | Light check |
| Before input, other tools | Crosshair | Heavy Mercedes | Mercedes | — | Intersection | Heavy checkmark | Checkmark |
| During input/editing | White pencil | Striped pencil, black top | Striped pencil | Tangent / perpendicular pencil | Intersection pencil | Filled pencil | Filled pencil, white top |
| Magic Wand | Empty-space wand | — | Edge wand | — | — | — | Node wand |
| Trim | Empty-space scissors | — | Edge scissors | — | — | — | — |
| Pick up parameters | Empty eyedropper | — | — | — | — | Full eyedropper | Half eyedropper |
| Transfer parameters | Empty syringe | — | — | — | — | Full syringe | Half syringe |

The same page separately lists empty-space, quick-selection, single/multiple-copy, multi-option, fill-handle, hammer, trident, 3D dimension-plane, and dimension-edge cursors. These remain outside this slice because OpenArchi has no corresponding modes yet.

## Feature contract

- **Goal:** Make cursor feedback respond to the active OpenArchi tool, plan/3D canvas, hit-test result, snap target, and active drag.
- **Entry/context:** Pointer enters the plan or 3D canvas. No element selection is required.
- **Interaction:** In the plan Select tool, element bodies use the ordinary arrow; editable nodes use the arrow with a checkmark; edges and Polyline segments use the arrow with a three-ray cue; empty space keeps the ordinary arrow. Node targets take priority over edges, which take priority over the element body. The arrow remains during Polyline Move Node dragging; middle-button view pan shows a grabbing hand. Select in 3D uses the ordinary arrow. Marquee shows a precision cursor. Wall, Slab, and Stair show a precision cursor while placing; resolved endpoint/midpoint snaps show a hand cue. Polyline uses a crosshair before its first point, then original hollow/solid pencil cues and a pencil-plus-line cue for angle/alignment snaps. Active pan, orbit, or element drag shows a grabbing cursor.
- **Inputs/defaults:** No settings or numeric inputs. Existing snapping and tool state determine feedback. Cursor changes do not change selection or model state.
- **Effects:** Plan and 3D derive cursor feedback from the same current project. Existing selection, snapping, undo/redo, and persistence behavior are unchanged.
- **Invalid/unavailable cases:** Empty space in Select uses the platform default. Invalid geometry does not gain an edit cursor. Locked/grouped/hotlink elements and sub-element handles are not modeled and therefore receive no special cursor.
- **Explicit exclusions:** This slice covers only existing tools and hit-test/snap states. The full Archicad cursor catalog (including Magic Wand, Trim, parameter pickup/transfer, dimensions, eyedropper/syringe, copy, multi-option, fill handle, and editable hotspots) requires those corresponding tools or model semantics first.

## Acceptance checklist

Launch the native app from the repository root with `./scripts/run.sh`.

1. With Select active, move over an element body; verify the ordinary arrow remains instead of a hand.
2. Hover a Polyline node and segment; verify the arrow gains a checkmark at the node and a three-ray mark on the segment. Hover a slab body and corner to verify body/node feedback also applies there.
3. With each drawing tool active, move over free/grid space and then a known endpoint/midpoint; the cursor distinguishes precision placement from a resolved snap.
4. With Marquee active, the plan and 3D canvases show precision feedback.
5. Pan the plan, orbit/pan 3D, and drag a selected element; the cursor shows grabbing while the operation is active.
6. Hovering changes no model data; existing click, drag, undo, and save behavior remains available.
7. Select a Polyline node to enter Move Node mode; verify the arrow remains while moving the node, then middle-button pan shows the grabbing hand.

## Verification record

`./scripts/cargo.sh test --locked --offline` and `./scripts/cargo.sh check --locked --offline --all-targets` are recorded in `docs/validation.md`. Formatting could not run because `cargo-fmt` is not installed for the selected stable toolchain. A geometry regression verifies node/edge/body priority, and a RawInput test verifies the arrow cursor output at a node. REA cursor-hover observation in Archicad remains unavailable; the matching Graphisoft AC29 public-help matrix is cited above. Exact command output and the runtime blocker are recorded in `docs/validation.md`.
