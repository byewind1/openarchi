# Research and dependency decisions — 2026-10-12

Official references checked during implementation:

- [Archicad 29 Toolbox](https://help.graphisoft.com/AC/29/INT/_AC29_Help/020_Configuration/020_Configuration-21.htm): grouped design tools; the prototype uses original Select/Wall/Slab tool drawings.
- [Archicad 29 Tracker](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-40.htm): numeric coordinate/distance entry during modeling. OpenArchi exposes length/angle or ΔX/ΔY with explicit commit.
- [Tracker and Coordinate Input](https://help.graphisoft.com/AC/29/INT/_AC29_Help/140_UserInterfaceDialogBoxes/140_UserInterfaceDialogBoxes-8.htm): contextual tracker and drawing aids.
- [Navigator concepts](https://community.graphisoft.com/t5/Getting-started/Archicad-Basic-Concepts-The-Navigator/ta-p/303987): navigation of project viewpoints. OpenArchi uses a story list plus synchronized plan/model views.
- [eframe official Rust API](https://docs.rs/eframe/0.36.2/eframe/): current stable release, native `App::ui`, wgpu integration.
- [egui-wgpu 0.36.2](https://docs.rs/egui-wgpu/0.36.2/egui_wgpu/): shared render state and native texture registration.
- [wgpu 30.0.1](https://docs.rs/wgpu/30.0.1/wgpu/): compatible version resolved by eframe; re-export avoids duplicate incompatible wgpu versions.
- [glam 0.34.1](https://docs.rs/glam/0.34.1/glam/): current stable camera math. DirectX-style 0..1 depth projection is used for wgpu (this is a clip-space convention, not a DirectX backend).

Locked dependencies: eframe/egui 0.36.2, wgpu 30.0.1, glam 0.34.1, serde 1.0.229, serde_json 1.0.154, bytemuck 1.25.2. Cargo.lock is included. eframe requires Rust 1.95 or newer; tested using installed stable Rust 1.98.1 on aarch64-apple-darwin. No prerelease crates were selected.

Agent Reach was consulted for research routing; its Exa backend was unavailable, so official documentation browsing and Cargo registry resolution were used. No Graphisoft trademarks, icons or interface assets are embedded in OpenArchi. Only workflow concepts are referenced.

## Marquee workflow — Archicad 29, 2026-10-12

### Evidence and confidence

- REA `open_binary` bound the installed `/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app` main executable as arm64 Mach-O, SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`. This establishes the local artifact identity, not its Marquee behavior.
- REA `observe_native_ui` captured the already-running Archicad 29 window (`PID 14896`, native window `38986`, title `Untitled`) with the accessibility tree and screenshot. Evidence: `ev_770776a6510da3aa8f57b589595eee6e1d7e8dfafb8b406f52d524a1f94f9c98`.
- REA `capture_native_ui_scenario` clicked the visible `[3D / Marquee, Story 0]` tab at AX path `[0,5,3]`; the action completed and the screenshot showed the same model in a perspective 3D view. Evidence: `ev_510054196715c40ed1a0d1a280994dfec325bb9c788ab5901868addbdd4a8111`.
- A follow-up REA `observe_native_ui` captured the resulting 3D window. Evidence: `ev_f1536c8c3569112a5aece083a4b1702754dde7408156a91ee52b9ad309bd0f52`.
- The 3D screenshot visibly shows two walls meeting at an orthogonal L corner, with no diagonal bevel face. This is direct visual evidence for that displayed model and view only; the zoom is too small to establish hidden-face, material-priority, or other junction rules. The screenshot does not reveal Archicad's internal geometry implementation.
- Marquee feature details below remain grounded in Graphisoft's public Archicad 29 help unless explicitly labeled as observed. Runtime behavior not captured here remains unknown.
- Feature details below are from Graphisoft’s public Archicad 29 help. Treat unlisted runtime details as unknown rather than inferred from the source code.

### Observed public workflow

- Marquee is a separate area selection, drawn with a dashed boundary; it does not itself create explicit element selection dots. `Cmd+A` while Arrow/Marquee is active makes enclosed elements explicitly selected. [Marquee Area](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-33.htm), [Special Selection Commands](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-23.htm)
- Floor Plan scope is Single Story or All Stories. Geometry methods are Polygonal (straight segments), Rectangle (two points), and Rotated Rectangle (three points). In 3D, the base is defined first, then its height; a fourth 3D-only mode draws a 2D region for copying an image. [Marquee Area Definition](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-34.htm)
- 3D additionally supports story filtering (all stories or a continuous range), optional trimming at the story-range boundaries, Inside/Outside Marquee effects, and element/custom attributes for cut surfaces. [Filter and Cut Elements in 3D](https://help.graphisoft.com/AC/29/INT/_AC29_Help/140_UserInterfaceDialogBoxes/140_UserInterfaceDialogBoxes-45.htm)
- For Cut/Copy/Clear/Drag/Mirror/Rotate/Elevate/Duplicate operations, an element needs at least one node inside the region; the operation acts on the whole element. [Other Editing Operations within the Marquee](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-37.htm)
- Stretch acts on linear elements whose endpoints are inside and polygonal elements with nodes inside. Enclosed nodes move along a vector; elements with all polygon nodes inside move as a whole. It is unavailable in the 3D window. [Stretching with the Marquee Tool](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-82.htm)
- Show Selection/Marquee in 3D includes any part of an element intersecting the plan area, even when no node is inside. Explicitly selected elements take precedence over the marquee. Marquee Effects can choose Inside/Outside and trim behavior. The installed workspace Navigator visibly exposed `Generic Perspective` and `Generic Axonometry`; this is a CUA-visible list, not an REA-captured runtime interaction. [Marquee Area](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-33.htm), [Filter and Cut Elements in 3D](https://help.graphisoft.com/AC/29/INT/_AC29_Help/140_UserInterfaceDialogBoxes/140_UserInterfaceDialogBoxes-45.htm)
- Remove the area with Esc, the context-menu command, a new area, or double-click outside. Esc clears explicit selections first; switching tools retains the area. [Marquee Area (AC30 help, same workflow)](https://help.graphisoft.com/AC/30/INT/Topics/030_Interaction/030_Interaction-33.html)
- A marquee can delimit PDF/DWG save and print output, and supports area copying from a project window. These are output operations, not model selection. [Save Marquee Area as PDF or DWG](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-35.htm)

### OpenArchi implementation and limits

The standalone Rust implementation lives in `editor.rs`, `geometry.rs`, `render.rs`, and `app.rs`. It provides plan and 3D perspective rectangle, polygon and rotated-rectangle base creation; right-click polygon closure; current-story/all-stories scope; numeric vertical depth; live 3D base preview; dashed region display; explicit `Cmd+A` selection by plan nodes; multi-select, group drag/delete/copy/cut/paste/duplicate/mirror/90-degree rotate/adjacent-story move, and node-based numeric stretch. The default is one plan window; F2/F3 switch between plan and perspective 3D. F5 gives the persistent Marquee priority—even after `Cmd+A`—and clips the shown volume; when no Marquee exists, it frames the explicit selection. This deliberately differs from Archicad's documented explicit-selection precedence to meet the requested cut-view workflow. The wgpu renderer triangulates simple concave polygons, clips wall/slab footprints and Z limits, and emits cap and cut faces.

Inside Marquee creates clipped meshes for the current vertical-prism wall and rectangular-slab primitives. Outside mode still filters whole elements and does not construct the geometric difference shell. The vertical depth is numeric rather than an Archicad-style second height-defining click; story-range trimming and custom cut-surface attributes are not implemented. Self-intersecting polygons, arbitrary non-prism BIM objects, the 3D-only 2D image-copy mode, native system clipboard interoperability, context-menu/double-click removal, PDF/DWG/print export, arbitrary-angle rotation of slabs, and non-model Archicad window types are not implemented. `Stretch` uses numeric ΔX/ΔY rather than Archicad’s interactive vector gesture. Internal copy/paste does not write the system clipboard.

## Polyline workflow — Archicad 29, 2026-10-14

The Kimi Code task was recovered from session `e2fb85ba-1093-45b8-abae-c17d8c52bc6c`, whose prompt was “读懂项目，用rea相关链路，复刻archicad的polyline的绘制功能” and whose final turn reason is `failed`. Its wire log shows the attempt stopped while trying to activate the toolbox item with screen coordinates; it had drafted an implementation checklist but did not modify source files for Polyline.

REA bound Archicad 29 ARM64 to executable SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`. Runtime evidence `ev_3f099f7142777dcb228abf6f1dc8851fffbd1b77f47a44b7655570a169225428` captures the active Polyline tool and the status text `Enter First Node of Polyline.` The visible Info Box shows `2D Drafting - General`, Solid line type, pen 62, Drafting Line, and three Geometry Method controls with the first selected. This evidence establishes only the active tool and displayed settings; it does not establish node creation, finish/close/cancel semantics, snapping, persistence, or 3D behavior.

The implementation contract and explicit unknowns are in [features/polyline.md](features/polyline.md). OpenArchi implements an original 2D drafting workflow with open finish and click-start closure; Enter/double-click finish and Escape cancel are product choices that remain unverified against Archicad runtime behavior. Filled regions, alternate geometry methods, arcs, pen/category controls, and 3D display are excluded from this slice.

## Polyline node editing — Archicad 29 public documentation, 2026-10-14

Graphisoft's [Pet Palettes help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/020_Configuration/020_Configuration-27.htm) documents opening a context palette by selecting an element, moving to an editable node/edge/surface, and clicking; command availability varies by element, hit type, and view, and the command can be changed before completion. [Move Nodes](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-91.htm) describes polygon node repositioning. [Adding Element Nodes](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-90.htm) documents the exact sequence: select the element, click an edge to open the Pet Palette, choose Insert new node, then click to set the point; double-click inserts without moving only while Insert Node is active. This documents insertion and node movement, but not the later OpenArchi panel layout. At the user's direction, OpenArchi uses a single-click-on-segment shortcut that inserts at the projected cursor position and opens its node-edit palette; this is a product-specific simplification, not a claim of Archicad's documented sequence. Graphisoft's [Reshaping Polygons and Chained Elements](https://helpcenter.graphisoft.com/user-guide/136853/) documents an **Offset edge** operation that offsets a whole side and stretches neighboring edges. AC29's separate [Offset All Edges](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-75.htm) applies to the whole polygonal element and describes a proportional increase/reduction preview. AC30's [Curve/Straighten Element Edge](https://help.graphisoft.com/AC/30/INT/Topics/030_Interaction/030_Interaction-92.html) documents Curve edge; the AC19 page specifies an arc through both segment endpoints and the dragged point. Therefore the requested whole-path constant-distance inward/outward offset is a user-directed OpenArchi behavior, not a verified match for AC29 Offset All Edges. OpenArchi offsets all path segments by a shared perpendicular distance and intersects adjacent shifted edges. The previous Curve edge implementation stored sampled vertices; it was replaced on 2026-10-15 by the exact arc-segment model documented below. Direct dragging of a selected Polyline body moves the whole element without a palette. Details and limits are in [Polyline editing](features/polyline-editing.md).

## Polyline exact circular arc segment — 2026-10-15

**Question:** while editing a Polyline curve edge, should pointer movement update the preview continuously, and should the model retain only the two edge endpoints plus circle parameters? The user explicitly specified yes. Graphisoft AC30's [Curve/Straighten Element Edge](https://help.graphisoft.com/AC/30/INT/Topics/030_Interaction/030_Interaction-92.html) documents dragging an edge to curve it; AC19's page says the arc passes through both endpoints and the dragged point. This supports the three-point circle behavior, but it does not establish Archicad's internal storage representation. No Archicad 29 drag was performed for this change, so runtime radius display and pointer thresholds remain unknown.

**OpenArchi design:** each edge optionally stores `ArcSegment { radius, clockwise, large_arc }`; the original vertex list remains the editable node list. Every drag frame recalculates the arc from the segment endpoints and pointer. Release commits one undo step. Display and edge hit testing use temporary tessellation; tessellation is never stored as model geometry. Selecting an existing curved edge opens edit context without inserting a vertex. Old JSON without the `arcs` field loads as all-straight edges. Radius feedback is shown in metres while the Curve edge drag is active.

**Automated coverage:** three-point circle and radius calculation, live radius change, unchanged endpoint count, JSON round-trip with arcs, and loading old JSON without arc metadata. Direct numeric radius entry and 3D Polyline display remain out of scope. Archicad runtime comparison remains unknown.

## Guides menu — Archicad 29 INT 5101, 2026-10-15

CUA runtime observation on the current Untitled ground-floor window enumerated all 19 Guides menu entries and observed Snap Point Values (3, 20.00%, 1000 mm); cancelled the dialog without geometry changes. REA bound the previously documented SHA-256 target and was closed after this investigation; CUA captures do not have REA Evidence IDs. No inferred IDs or unobserved Archicad drawing/undo results are claimed. Graphisoft AC29 official Snap Points, Snap Guides, Guide Lines and Distance Guides documentation supplies the behavior semantics. [The focused contract](features/guides.md) separates these evidence types and records plan-only implementation boundaries.

Original source: `guides.rs` owns typed settings, bounded marker generation, exact arc interpolation and isolated intersections; `distance_guides.rs` computes wall clearances and validated movement/stretch; `guides_ui.rs` integrates native menus, pointer previews, erasing, numeric entry and history. Old JSON imports use defaults. Door/window choices are disabled because the current model has no openings; this is partial coverage of Archicad's full Guides ecosystem, not equivalence of all element/view workflows.

## Trace & Reference — Archicad 29 INT 5101, 2026-10-17

CUA observed the running Archicad 29 Untitled project (target SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`) and enumerated the Standard Toolbar `Trace` toggle and its drop-down: `Choose Reference` (`Browse for Trace Reference…`, `Above Current Story`, `Below Current Story`, `Previous Story`, `Entire Model Display`), then `Activate Reference`, `Switch Reference with Active`, `Drag Reference`, `Rotate Reference`, `Reset to Default Position`, `Rebuild Reference`, `Temporarily displace Reference`, `Show/Hide Splitter`, `Reference on Top`, `Transparent Fills and Zones`, `Turn off Trace everywhere`, `Trace & Reference`. The palette showed `Reference:` / `Active:` intensity sliders at 255. A later CUA capture after selecting `Above Current Story` failed with `invalid screen capture size`; no default reference placement or pixel appearance is claimed. CUA captures here have no REA Evidence IDs.

Graphisoft's public AC29 help pages `030_Interaction-103` through `112` define the semantics: one Reference per floor-plan window shared across stories; Reference elements are visible but not directly editable; `Activate Reference`, `Switch Reference with Active`, `Drag`, `Rotate`, `Reset`, `Rebuild`, `Temporarily displace`, the splitter, Reference on Top and Transparent Fills are display/edit aids; `Previous Story` is the last story visited. These are documented behavior, not locally reproduced runtime observations.

**OpenArchi design:** `src/trace.rs` owns a typed `TraceState` (relative/absolute/entire-model reference, intensity, uniform color, visibility, offset/rotation, pointer modes, splitter) and deterministic story resolution; `src/trace_ui.rs` integrates the toolbar drop-down, `View > Trace Options`, `Window > Trace & Reference` palette, browse dialog and plan painting. Reference drawing is derived from the live project, uses plan footprint/outline geometry per material, supports type filtering and transparent fills, and is clip-order aware with Active content. Drag/Rotate/Displace operate on the display transform only. Trace state is not part of the project JSON and creates no undo entry or dirty flag. The splitter is a shared-viewport swipe (Reference left, Active right), not two independently navigable views. Reference snapping and non-plan reference types are explicit gaps.

**Automated coverage:** relative story resolution including elevation ordering, previous story, entire model, transform/reset, pointer-mode cancellation, trace toggle fallback, Switch with Active leaving model and undo untouched, RawInput drag changing only the Reference, Escape restoring temporary displacement, and palette/browse/splitter rendering without model mutation.

## Quick Options — Archicad 29 INT 5101, 2026-10-17

Graphisoft's AC29 help page `020_Configuration/020_Configuration-20.htm` documents Quick Options as a bottom bar whose contents reflect the active tab. A local AC29 29.0.0 v1 INT (5101) capture on 2026-10-17 shows the bar docked above the status line, left-to-right: Zoom, Orientation, Scale, Layer Combination, Partial Structure, Pen Set, Model View Options, Graphic Overrides, Renovation Filter, Design Options and Dimensions. The same build's `Window` menu contains both `Hide/Show Quick Options Bar` and a separate `Window > Palettes > Quick Options` floating palette. These are screenshot/menu observations without REA Evidence IDs. The capture's `02 Drafting` sits beside the stacked-layer glyph and is a Layer Combination; `03 Architectural 100` sits beside the pen glyph and is a Pen Set. AC29 icon artwork remains proprietary and is not copied.

**OpenArchi design:** `src/app.rs::quick_options_bar` renders the eleven AC29-ordered controls with original vector glyphs above the status bar. `quick_options_palette` renders the same settings in an independent floating window. Both call the same `set_*` commands and menu contents used by `Document` and the relevant `Options` entries. Zoom is view-only; the other controls edit `Project::plan_options` through `modify_plan_options`, which validates, commits one undo step and reports the new value. Layer combinations, partial structure, pen palettes, model-view options, graphic overrides, renovation filters and dimension styles gate or style plan painting. Design options are stored on elements; Main Model Only hides elements assigned to an option, while an active option also shows main-model elements. Bar and palette visibility are session UI state and do not mutate the project or undo history.

**Automated coverage:** regressions cover shared plan-option editing and undo for seven controls, layer/design-option visibility, independent bar/palette visibility without model mutation, zoom label mapping, and world/screen orientation round-trip. Full-suite counts and the final package UUID are recorded in [Validation](validation.md) after the build. Evidence types, the finite checklist and remaining unknowns are in [the focused contract](features/quick-options.md).

## Wall settings — Archicad 29 INT 5101, 2026-10-18

CUA observed the running Archicad 29 `Untitled` project with the Wall tool active. `⌘T` opened `Wall Default Settings`; the target identity was `29.0.0 v1 INT (5101)`, SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`. The capture enumerated the six top-level groups and, after expanding each group, the geometry, floor-plan, model, structural, and classification rows listed in [Wall settings](features/wall-settings.md). The dialog was dismissed with `Cancel`; no Archicad project data was written. CUA captures have no REA Evidence IDs, so this is a runtime UI observation rather than artifact analysis.

**OpenArchi design:** `src/wall.rs` owns the typed `WallSettings` record and validation; `src/wall_ui.rs` is the reusable AC29-style editor; `Element.wall_settings` persists the record with `#[serde(default)]` compatibility. Geometry fields (height, thickness, reference offset, story and start/end) update the shared Plan/3D model; top/bottom offsets shift the 3D solid. Profile modifiers, floor-plan display overrides, surfaces/textures, structural analytical parameters and classification values are stored and validated but do not yet change geometry or rendering. `⌘T` routes selected walls to Wall Settings and the active Wall tool without a selection to Wall Default Settings. See the feature contract for the exact supported/unsupported boundary.

## Wall geometry methods — Archicad 29 INT 5101, 2026-10-18

Graphisoft's AC29 help defines four distinct input methods: Single Straight Wall from two endpoints, Chained Wall from successive segments completed by double-click, Rectangle Wall from a diagonal whose four sides stay orthogonal to the Normal Grid, and Polygon Wall from a free contour with one reference edge. The help pages are public documentation; this pass did not capture the four gestures as REA runtime Evidence.

**OpenArchi design:** `WallGeometryMethod` now selects a real input state machine in `src/editor.rs`. Single is two clicks. Continuous stores successive nodes, supports Backspace, Enter and double-click completion, and commits the chain once. Rectangular emits four world-axis-aligned segments from two corners and uses a ΔX/ΔY Tracker. Polygonal normalizes a closed ring counter-clockwise and commits its segments once. Shared plan previews, method switching, cancellation and localized status are wired in `src/app.rs`. The current model has no PolyWall entity, so non-Single methods produce groups of ordinary wall segments; curved pet-palette segments, Rotated Rectangle, a movable polygon reference edge, self-intersection warnings and single-element PolyWall semantics remain explicit gaps. See [wall geometry methods](features/wall-geometry-methods.md).

## PLN / Archicad 图库互操作提案 — 2026-10-20

用户要求先评估当前架构，再写实施方案，核准后实施。[架构及实施方案](features/pln-interop/plan.md)建议先验证依赖本机 AC29 的原生编辑桥接，使用源工程副本与字段白名单回写；独立 Rust PLN 编解码和完整 GDL 运行时另设研究门槛。[证据台账](features/pln-interop/evidence.md)记录当前源码、固定 AC29 官方 API 文档、本机目标身份与 REA 限制。本轮没有解码 PLN、转换图库或实现功能，不能宣称已支持这些格式。

**REA MCP 复查（2026-10-20）：** 搜到本机两份 PLN 候选样本，记录大小与 hash 后尝试走当前 `open_binary + Ghidra`。REA 明确拒绝样本：`unsupported binary format`；AC29 29 INT 5101 Mach-O 本身成功绑定 Ghidra 12.1.4，但 overview 与关键字符串/函数搜索均长时间无返回，停止等待后未取得 Evidence。close 调用未及时返回，但稍后的最终状态确认本次 Ghidra 会话已关闭。故当前链路证明的是 PLN 不在此 Ghidra target 路由的支持格式内；没有证明其内部结构，也不等于整个 REA 永无其他读取方式。详情见证据台账当日补记。

REA 6.4.0 CLI 随后也拒绝 test1.pln 为 unsupported format。shell 不会自动继承 Codex MCP 的 Ghidra env；按已有工作流显式带入安装/JDK 路径后，对 AC29 的搜索运行约 210 秒仍未完成，Ctrl+C 后 REA 报告 cancelled 且 cleanup complete。REA 当前认可的 binary-layout 命令只接受 Linux ELF。没有返回 PLN 内部结构结果；完整路径与边界见 [证据台账](features/pln-interop/evidence.md)。

## Parameter Transfer — Archicad 29 INT 5101, 2026-10-24

Graphisoft's public AC29 help (`030_Interaction-98/99`) defines Parameter Transfer: hold Alt (Option) and click an element to pick up its parameters as the tool default; hold Ctrl+Alt (Option+Cmd) and click to inject the picked parameters into same-type elements; Line/Arc/Polyline/Spline form one 2D drafting type; the Element Transfer Settings dialog stores named transfer sets with per-type checkboxes, and unlisted settings always transfer. A user-supplied screenshot of the running AC29 toolbar confirmed the "Pick Up Parameters (Option+C)" tooltip. The exact always-transferred list, the ⌘⌥C command shortcut (third-party tables only) and 3D gesture details remain unknown; no REA runtime Evidence was captured for this feature.

**OpenArchi design (original implementation):** `src/transfer.rs` owns `TransferType`/`TransferSet`/`TransferSettings` and `apply_transfer` with five all-or-nothing groups (Geometry, Material & Layer, Surfaces, Floor Plan Display, Data) — identity, position, story, renovation and design option never transfer. `Editor::pick_up_from` stores one snapshot per type (session state, no undo) and mirrors picked values into tool defaults; `Editor::inject_into` validates the whole batch and commits one undo record, or leaves the project untouched on failure. `src/transfer_ui.rs` adds the Edit-menu commands, `⌥C`/`⌘⌥C`, one-shot armed cursor modes with Escape cancellation, and the Element Transfer Settings dialog (sets stored on `Project.transfer_settings`, serde-default for old JSON). Gestures (`⌥+click` pickup, `⌘⌥+click` inject) work in both plan and 3D views.

**Verification:** 14 focused tests (9 unit + 5 egui regressions) pass inside the 274-test suite; localization has 472 keys with both locales complete. Desktop interaction verification was blocked by the session environment — the ZCode host lacks Accessibility (CGEvent posts leave the cursor untouched, osascript reports -25211), app activation is suppressed from this background context, and CuaDriver refuses background input for AX-unresolvable egui windows. See [Validation 2026-10-24](validation.md) for the full blocker record; the contract and remaining unknowns are in [parameter transfer](features/parameter-transfer.md).

## Archicad 模块复刻难度评估 — 2026-10-25

### 结论与估算口径

REA 显著降低的是行为发现成本：它可帮助定位入口、控件状态、调用路径，并对限定场景做运行时观察。它不会自动提供 OpenArchi 的数据模型、几何算法、视图派生、历史系统、持久化或格式兼容。模块难度必须分开估算为“UI/交互复现”和“模型语义/全链路复现”。以下难度是面向单机、独立模型、逐步实现的工程判断，不是对 Graphisoft 内部实现的测量。

| 难度 | 模块类型 | 主要成本 |
|---|---|---|
| 低到中 | 菜单、工具栏、简单对话框、偏好与视图设置、简单属性编辑 | 状态有限；需复现校验和提交/取消，但与几何关系较少。 |
| 中到高 | 墙、板、门窗、楼梯等构件工具；捕捉、约束、拖动和属性修改 | 要连接工具状态机、输入约束、模型变更、选择、平面/3D、撤销和持久化。 |
| 高 | 多构件连接、复合构造、宿主洞口、剖立面/尺寸/表格等文档视图 | 需要维护跨元素关系并在编辑、视图、属性统计和重开后保持一致。 |
| 很高 | PLN/GSM 互操作、完整图库语义、Teamwork、大项目性能及全面兼容 | 涉及私有/版本化格式、参数化运行时、服务端权限与冲突，或规模下的计算成本。 |

### 本机仓库与目标证据

- 本机目标身份：Archicad 29.0.0 v1 INT (5101)，Apple Silicon；可执行文件 SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`。本次复用既有界面盘点与证据，没有重新操作目标应用。
- `docs/archicad-interface-inventory.md` 记录了本机 14 个顶层菜单及 1272 行辅助功能树；这表明可观察界面面很广，不能证明命令背后的模型行为已被调查或实现。
- `README.md` 当前列出的未实现项包括通用布尔、约束求解、剖立面、施工图、协作、工程量扣减，以及 PLN/GSM/BSM 解析；当前适配器只有自有 JSON。墙设置也明确区分“字段可保存”和“轮廓生成/表面映射/结构求解已接入”。这是仓库状态描述，不是 AC29 功能清单。
- `docs/performance-baseline.md` 记录 50,000 面墙的轮廓准备约 133–155 秒，提示大项目性能本身已是实际难题；不应从小样本行为正确推断可扩展性。
- 本机 PLN 调查见 [PLN REA 限制记录](features/pln-interop/rea-ghidra-unsupported-format.md)：REA 6.4.0 在可执行文件目标解析阶段拒绝 PLN；Ghidra 强制 raw import 仅证明可装入原始字节，不提供 PLN 语义。此结论针对记录的工具链和样本，不声称所有可能格式研究路线均无效。

### 官方资料与社区旁证

- Graphisoft AC29 API 的 [`API_ElementMemo`](https://graphisoft.github.io/archicad-api-devkit/struct_a_p_i___element_memo.html) 包含多边形坐标/圆弧、孔洞、库部件参数、屋面/壳体轮廓、楼梯分段等不同可变数据。这支持“UI 控件数量不是模型实现工作量”的判断；API 文档本身不测量独立实现成本。
- Graphisoft 的 [Teamwork 功能概览](https://help.graphisoft.com/AC/29/INT/_AC29_Help/080_Collaboration/080_Collaboration-3.htm)描述元素和项目数据保留、在线状态反馈、所有权冲突请求、增量 Send/Receive、离线本地副本及用户权限等。故协作应视为独立系统能力，而非菜单外观工作。
- Reddit 上的 [Archicad JSON Interface / MCP 开发者讨论](https://www.reddit.com/r/ArchiCAD/comments/1t9a9ok/has_anyone_built_an_mcp_server_for_archicad_json/)谈到接口覆盖、批量操作和上下文管理；这是用户经验，只作旁证，不作为 API 保证或产品事实。
- agent-reach 的 Exa 后端未配置；公开检索改用已可用的 Reddit/GitHub 路由和网页搜索。讨论未被当作权威技术证据。

### 建议的难度判断

若目标是一个实用的单用户 BIM 子集，优先完成少数垂直功能切片，并把数据模型、视图一致性、撤销/保存和性能作为共同底座；REA 可以让每个切片的 Archicad 行为调查更省力。若目标改为 Archicad 全模块等价，原生格式、图库运行时、剖面/文档系统、协作和规模性能会形成多个独立的长期工程，不应按菜单项数量线性拆分估时。

**未知与限制：**本轮没有重新捕获 Archicad 运行时动作；没有对 API 覆盖率做系统盘点；没有进行 PLN/GSM 格式解析；社区经验不保证适用于所有项目或 AC29 build。难度分档是结合仓库当前边界、公开 API/帮助与工程推断。

**同步日期：**2026-10-25。Obsidian 副本：[OpenArchi 模块复刻难度评估（2026-10-25）](</Users/ren/Library/Mobile Documents/iCloud~md~obsidian/Documents/库/01-Projects/dev开发/OpenArchi 开发/OpenArchi-模块复刻难度评估-2026-10-25.md>)。仓库根 `AGENTS.md` 的 “Archicad module effort assessment” 同步保存可复用评估原则。


## 2026-10-28 — Edit 鼠标交互纠正与全量标准快捷键

[交互契约](features/edit-menu/pointer-interaction.md)区分入口组合键与操作中修饰键，引用 AC29 官方 Drag/Rotate/Mirror、复制、Tracker 和取消操作帮助。当前数值对话框不是已完成的鼠标操作复刻。记录用户 Ctrl 描述与官方 macOS Cmd+Option 的差异、镜像多副本和 key-up/Undo 边界未知项。[标准配置全表](features/edit-menu/standard-shortcuts-full-29.md)保存本机 XML 全部 171 条绑定（Global 115 / Non-Text 47 / Text 9），含源文件 hash、上下文和机器可读数据。已同步指定 Obsidian OpenArchi 开发目录；未修改应用源码或进行新打包。


## 2026-10-29 — 操作研究与墙编辑技能沉淀

用户要求将本轮方法、真实判断、开发教训与规则沉淀。已更新 [AC29技能入口](../skills/openarchi-ac29/SKILL.md)、[操作研究参考](../skills/openarchi-ac29/references/operation-research.md)、[实现技巧](../skills/openarchi-ac29/references/implementation.md)及AGENTS复盘节。证据类型逐主张区分；Cmd Trim与十图标的文档/推断边界、多选节点用户要求、AC29 Fan4自身尖片与OpenArchi修复选择均明确标注。未补做锁屏阻断的最终包原生验收。

完整中文复盘保存于用户指定Obsidian目录：`OpenArchi-Archicad操作研究与墙编辑开发技能沉淀-2026-10-29.md`。本次仅更新知识与技能，没有新增应用源码改动；技能quick_validate和diff检查通过。
