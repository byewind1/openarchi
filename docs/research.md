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
