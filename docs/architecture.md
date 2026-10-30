# OpenArchi architecture

> 2026-10-31 规划更新：目标是从方案到施工图、AI 原生可调用的独立工作台。当前架构盘点、已知文档漂移及目标分层见 [架构与分阶段开发计划](features/ai-native-workbench/plan-2026-10-31.md)。下文为历史实现概览；“无墙连接”“板仅矩形”等限制已落后于当前工作树，不能用它们否定新实现或推断已验收范围。

OpenArchi is a native Rust executable. Cargo retains the internal package/library identifiers `atelier-bim` / `atelier_bim` to preserve code dependency compatibility; the shipped binary, app bundle and project extension are named `openarchi`. No browser, HTML, JavaScript runtime or WebView is part of the application.

- `model.rs`: versioned independent project, stories, stable element IDs, wall/slab parameters, hosted rectangular door/window openings, materials, validation, and `ModelAdapter` interface. Openings store `host_wall`, `kind`, `width`, `height`, `sill` and centre `offset`; validation checks host, story, wall bounds, vertical bounds and overlap.
- `editor.rs`: tool state machine, wall/slab/opening creation, marquee area and scope, multi-selection, group transforms, node stretch, transactional edits, typed command application, bounded reversible-delta undo/redo (100 transactions), dirty tracking. Opening drags clamp to the host wall; deleting a wall cascades to its openings in one transaction, and paste remaps opening hosts only when the host wall is copied too.
- `command.rs`: `ModelCommand` (`AddElement`, `RemoveElements`, `RestoreElements`, `UpdateElement`, `MoveElements`, `SetOptions`, transitional `ChangeSet`), `CommandRecord { command, inverse }`, and `ProjectDelta` element/header diffs used by undo and redo.
- `geometry.rs`: wall footprints, extrusion corners, plan hit testing, simple-polygon triangulation and prism intersection, marquee node-vs-overlap selection, endpoint/midpoint/grid snapping, perspective camera projection and triangle ray picking.
- `render.rs` / `scene.wgsl`: wgpu mesh and edge pipelines, perspective camera, true inside-Marquee mesh clipping and cut faces for walls/slabs, outside-volume element filtering, 4× MSAA, offscreen color texture and Depth32Float buffer, texture registration with egui. eframe chooses Metal on macOS.
- `app/mod.rs`: `AtelierApp` state, constructors, styling/localization helpers, workspace entry and the `eframe::App` implementation. The application is split into the modules below; line counts are not an architectural invariant.
- `app/shell.rs`: title bar, menus, toolbar, toolbox and status bar.
- `app/plan.rs`: plan canvas, pointer interaction, Tracker, snapping, hit testing, marquee and polyline editing.
- `app/model_view.rs`: perspective 3D viewport rendering and camera interaction.
- `app/inspector.rs`: properties, library and local assistant panels.
- `app/info_box.rs`: Info Box selection state and wall/tool property widgets.
- `app/quick_options.rs`: Quick Options bar and palette, plan-option menus and zoom/orientation controls.
- `app/dialogs.rs`: element, wall, stair, command-preview, plan-options and design-options dialogs.
- `app/file_io.rs`: native file dialogs, save/open actions, dirty-close guard and recovery prompts.
- `app/ui_tests.rs`: headless egui interaction and rendering regressions.
- `storage.rs`: validated JSON adapter, file size limit, temporary-file + rename save.
- `fonts.rs`: platform CJK font discovery (including Microsoft YaHei, SimHei, SimSun and Deng on Windows) with a bundled Noto Sans SC fallback under the SIL Open Font License.
- `recovery.rs`: validated crash-recovery JSON sidecar in the platform configuration directory; written only for dirty projects, cleared on save/discard.

Both views read the same `Project`; there is no duplicate 3D scene graph or format-specific project state. Mesh generation reads extruded domain elements on each requested frame. `Editor::apply` records the affected elements before and after a typed command. Operations not yet migrated still call `commit(before)`, which stores only the changed elements and project headers rather than another full project. Dragging previews on the current project and commits one transaction on release.

Future IFC, GSM and BSM integration should implement adapter/plugin boundaries that convert into domain objects, with external IDs and metadata added as explicit extensible records. The current `ModelAdapter` only defines a byte import/export contract; a real GSM/BSM runtime would need a richer parametric-object/provider interface. No actual external format parsing is implemented or implied.

The [PLN and Archicad library interoperability proposal](features/pln-interop/plan.md) (2026-10-20, awaiting approval) evaluates an AC29 Add-On bridge, session-based document backends, library providers, source-baseline patches, and a separate independent PLN format research track. These are proposed boundaries, not implemented capabilities; [evidence and limitations](features/pln-interop/evidence.md) distinguish source analysis, official AC29 documentation, and incomplete REA investigation.

## Deliberate limits

- Walls remain independent model elements. `wall_geometry.rs` derives shared plan/3D joins for supported straight-wall contexts; curved/custom contours have separate bounded paths. Compounds and general constraint solving are not implemented. The only hosted feature is a rectangular door/window opening on one straight wall; curved walls, non-rectangular openings, frames/sashes and junction-crossing cuts are not modelled. 3D emits piers plus sill/lintel blocks and a thin glazing pane for windows, and excludes walls with openings from the joined-wall outline optimisation.
- Slab creation starts from a rectangle; optional `SlabContour` stores arbitrary outer rings, holes, exact edge arcs, edge angle/surface settings and elevation. `slab_geometry.rs` serves plan/3D/picking/transforms. Boolean editing uses sampled topology with arc recovery. Full native mouse acceptance and sloped-edge clipping remain unknown; see [scope and evidence](features/slab-edge-editing/contract.md).
- Explicit multi-selection is separate from the persistent marquee region; property edits target one selected element. Plan supports marquee region editing and group operations. In perspective 3D, rectangle bases are dragged and polygonal or rotated-rectangle bases are placed on the current story plane by screen-ray intersection; polygon closes on secondary click. The numeric Info Box depth defines the vertical range. Shift+middle-drag orbits, middle-drag pans. F2 switches to plan, F3 to perspective 3D, and F5 prioritizes and clips the persistent Marquee even when explicit selection exists; without a Marquee, it shows selected elements.
- Command+T switches the inspector to the Properties tab and opens its right panel. The selected wall/slab draft exposes its name, home story, material, and primitive dimensions; Apply validates and commits one model snapshot so both views update together.
- Inside Marquee clips supported vertical-prism walls and rectangular slabs using simple polygon XY intersection and Z limits. Outside mode only filters elements; it does not construct a subtractive shell. Self-intersecting polygons and other BIM primitives are outside the mesh pipeline. There are no shadows, PBR, ambient occlusion, sections or render export. CPU ray picking is linear in element count.
- Geometry buffers are rebuilt on repaint; no spatial index, instancing or incremental mesh cache. `docs/performance-baseline.md` records 1k/10k/50k wall baselines and shows wall-outline preparation as the dominant cost. The 32 MB JSON and 100,000-element validation caps are input guardrails, not a supported performance promise.
- Positions use f32 metres; suitable for a local prototype, not georeferenced or survey-scale precision.
- The assistant is a clearly labeled offline rules interface; no LLM or external service is called.
- Presets are built-in wall/slab settings, not an external object library.
- Undo history is bounded to 100 records and stores diffs, but complex operations that have not migrated to `Editor::apply` still clone the project once to compute their diff. Crash recovery is a single 60-second sidecar, not collaborative or professional document production.
- Layout can be resized and toggled during a session; arbitrary docking and persistent layouts are not implemented.
- Native close uses a Save/Discard/Cancel guard for dirty projects. Crash recovery is a single last-writer-wins sidecar, not a versioned autosave history; selection, camera and panel state are not recovered.
- Save/Open use the native `rfd` panel with the path field retained as a fallback. The native panel itself is not covered by headless tests.
