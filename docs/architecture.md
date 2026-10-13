# OpenArchi architecture

OpenArchi is a native Rust executable. Cargo retains the internal package/library identifiers `atelier-bim` / `atelier_bim` to preserve code dependency compatibility; the shipped binary, app bundle and project extension are named `openarchi`. No browser, HTML, JavaScript runtime or WebView is part of the application.

- `model.rs`: versioned independent project, stories, stable element IDs, wall/slab parameters, materials, validation, and `ModelAdapter` interface.
- `editor.rs`: tool state machine, wall/slab creation, marquee area and scope, multi-selection, group transforms, node stretch, transactional edits, bounded snapshot undo/redo (100 transactions), dirty tracking.
- `geometry.rs`: wall footprints, extrusion corners, plan hit testing, simple-polygon triangulation and prism intersection, marquee node-vs-overlap selection, endpoint/midpoint/grid snapping, perspective camera projection and triangle ray picking.
- `render.rs` / `scene.wgsl`: wgpu mesh and edge pipelines, perspective camera, true inside-Marquee mesh clipping and cut faces for walls/slabs, outside-volume element filtering, 4× MSAA, offscreen color texture and Depth32Float buffer, texture registration with egui. eframe chooses Metal on macOS.
- `app.rs`: original egui product shell, resizable panels, plan canvas, Tracker, properties, presets, local assistant, JSON file UI.
- `storage.rs`: validated JSON adapter, file size limit, temporary-file + rename save.

Both views read the same `Project`; there is no duplicate 3D scene graph or format-specific project state. Mesh generation reads extruded domain elements on each requested frame. Drawing and property edits enter undo history as project snapshots; dragging previews on the current project and commits one transaction on release.

Future IFC, GSM and BSM integration should implement adapter/plugin boundaries that convert into domain objects, with external IDs and metadata added as explicit extensible records. The current `ModelAdapter` only defines a byte import/export contract; a real GSM/BSM runtime would need a richer parametric-object/provider interface. No actual external format parsing is implemented or implied.

## Deliberate limits

- Walls are independent straight prisms, with no boolean joins, hosted openings, compounds, constraints or curved segments.
- Slabs are axis-aligned rectangles with uniform thickness, no holes or arbitrary outlines. Arbitrary-angle rotation is not supported.
- Explicit multi-selection is separate from the persistent marquee region; property edits target one selected element. Plan supports marquee region editing and group operations. In perspective 3D, rectangle bases are dragged and polygonal or rotated-rectangle bases are placed on the current story plane by screen-ray intersection; polygon closes on secondary click. The numeric Info Box depth defines the vertical range. Shift+middle-drag orbits, middle-drag pans. F2 switches to plan, F3 to perspective 3D, and F5 prioritizes and clips the persistent Marquee even when explicit selection exists; without a Marquee, it shows selected elements.
- Command+T switches the inspector to the Properties tab and opens its right panel. The selected wall/slab draft exposes its name, home story, material, and primitive dimensions; Apply validates and commits one model snapshot so both views update together.
- Inside Marquee clips supported vertical-prism walls and rectangular slabs using simple polygon XY intersection and Z limits. Outside mode only filters elements; it does not construct a subtractive shell. Self-intersecting polygons and other BIM primitives are outside the mesh pipeline. There are no shadows, PBR, ambient occlusion, sections or render export. CPU ray picking is linear in element count.
- Geometry buffers are rebuilt on repaint; no spatial index, instancing, incremental mesh cache or large-project benchmarks. The 32 MB JSON and 100,000-element validation caps are input guardrails, not a supported performance promise.
- Positions use f32 metres; suitable for a local prototype, not georeferenced or survey-scale precision.
- The assistant is a clearly labeled offline rules interface; no LLM or external service is called.
- Presets are built-in wall/slab settings, not an external object library.
- Snapshot history can be expensive on large projects. No autosave, crash recovery, collaboration or professional document production.
- Layout can be resized and toggled during a session; arbitrary docking and persistent layouts are not implemented.
- Native window close does not yet provide an unsaved-changes guard; use Save before closing.
