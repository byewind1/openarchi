# Guides menu — Archicad 29 reference / OpenArchi implementation

## Scope and evidence (2026-10-15)

Goal: implement the standard toolbar Guides menu in the independent native OpenArchi model. Reference: running Archicad 29.0.0 v1 INT (5101), arm64 macOS, executable `/Applications/GRAPHISOFT/Archicad 29/Archicad 29.app/Contents/MacOS/Archicad`, SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505` (REA open_binary). No existing REA session was open; this investigation opened its own binding.

Runtime observation via CUA, Untitled project, ground-floor plan, Arrow selected: opened Guides toolbar menu. Observed Snap Guides and Points; Half; Divisions (3); Percent (20.00%); Distance (1000 mm); Best Division by Distance (1000 mm); Set Snap Point Values; Between Nodes; Between Intersection Points; Along Entire Element; Guide Lines; Create Guide Line Segment; Remove All Guide Lines; Erase Guide Lines; Distance Guides; To reference lines; To edges; To door/window nominal size endpoints; To door/window center. The three Guide Line operations were disabled in the captured state. Opened Snap Point Values: divisions 3, percent 20.00, distance 1000; Escape cancelled. No project geometry changed. These CUA observations have no REA Evidence ID and prove only menu/dialog presence and displayed values, not creation/undo semantics. REA observation requires a numeric native window ID not exposed by the available macOS CUA inventory; no fabricated runtime Evidence is claimed.

Public documentation (AC29):
- [Snap Points](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-48.htm)
- [Snap Guides](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-46.htm)
- [Guide Lines](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-47.htm)
- [Distance Guides](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-65.htm)

See geometry and distance companion notes for algorithms. These sources establish documented behavior, not a measured comparison of every Archicad interaction.

## Contract / reversible implementation choices

- Toolbar Guides and View menu use the same typed state. Floor-plan drafting uses metres (UI fields say m); percentages are percent. Settings dialog edits a draft, validates before Apply, Cancel/Escape leave saved settings unchanged. Settings and fixed guides are serialized with backward-compatible defaults; changes participate in project undo/redo.
- Snap points: five interval modes, three ranges, endpoint/midpoint origin; point markers on hovered geometry feed the same drawing target used by walls/slabs/stairs/polylines. Element snap and grid snap remain separately controlled. Temporary references use Q, clear at operation completion/Escape; fixed guide visibility is independent.
- Fixed guides: straight segment and circle construction, two clicks with preview, Escape cancels; edge tabs create infinite lines; handles drag/offset; eraser clicks/drags remove guides, Escape exits; Remove All spans stories. Geometry is validated and undoable. Guides do not become building elements or change quantities.
- Distance guides: plan wall input and selected straight wall editing; reference-line/physical-edge modes. Numeric distance editing produces validated shared model changes and one undo transaction. No door/window, beam, column or grid-element model exists yet; corresponding unavailable references must be disabled/explained, not simulated.
- Initial scope is the floor-plan workplane; unrestricted 3D guide placement, full Archicad relative/tangent/bisector inference, contextual relinking parity and native project-format persistence remain declared gaps unless separately verified. Original UI/assets only.

## Acceptance checklist

1. Open Guides and exercise every enabled menu item; settings Cancel, invalid values and Apply.
2. Known straight and curved paths: midpoint, 3 divisions, 20%, 1 m steps and best-fit parts; all ranges, reversed endpoint and midpoint origins.
3. Place guide segment/circle; cancel preview; hide/show; snap drawing; move/radial offset; erase; remove all across stories; undo/redo; save/reopen.
4. Toggle snap aids independently; Q reference and Shift projection; no accidental model placement while guide editing.
5. Walls at known spacing: reference/edge distances, numeric edit, validation, undo/redo, plan/3D shared state.
6. Format/check/full relevant tests, fresh packaged app UUID and signature; native smoke of current bundle. No global equivalence claim.

Verification results are appended after execution.

## Implemented coverage and declared comparison limits

| Menu group | OpenArchi acceptance | Archicad equivalence evidence |
|---|---|---|
| Snap Guides and Points | Independent toggle; hovered marker lifetime; Q point/edge references; Shift projection; parallel/perpendicular references | Menu observed; documentation. Full tangent/bisector/increment-angle inference remains incomplete |
| Half / Divisions / Percent / Distance / Best Division | Exact path-length tests, nearest-endpoint origin, 3–100 division validation, bounded dense markers | Menu + settings defaults observed; documented semantics; runtime output coordinates unknown |
| Set Snap Point Values | Draft modal, invalid Apply rejected, valid Apply one undo step, Cancel/Escape unchanged | Dialog observed, cancelled; Archicad validation ranges from public documentation |
| Between Nodes / Intersection Points / Entire Element | Straight and exact arc cases pass expected-coordinate tests | Documented scopes; no matching Archicad measured model comparison |
| Guide Lines / Create Segment | Visibility separate from snapping; segment/line/circle/three-point arc; edge tabs; handles; signed numeric offset; preview/cancel | Menu and public docs; OpenArchi palette is original, arc three-click method is a product choice |
| Remove All / Erase | All stories; click/swept eraser; one transaction per gesture; undo/redo | Menu and public docs; no Archicad undo replay |
| Distance Guides / To reference lines / To edges | Current-story straight walls, physical edge clearance, numeric wall input and selected-wall move/endpoint stretch | Documented workflow; projection and editing tested in independent model |
| Door/window endpoints / center | Disabled with missing-model explanation | Menu observed; unavailable model capability, not claimed implemented |

Additional explicit gaps: arbitrary 3D guide editing/display, guide-circle-to-line conversion on drop, radius-only drag of guide arcs (arcs currently translate; numeric offset is concentric), distance-arrow contextual relinking, beam/column/grid-element contexts, all Archicad hover-delay/preferences and context-menu pinning variants. `L` remains OpenArchi's existing slab shortcut; no shortcut collision introduced. `D` opens a frozen nearest wall-distance input in plan. Fixed guides remain view aids and never enter building mesh/quantities. Settings persistence/undo is an original OpenArchi choice; Archicad settings persistence was not measured.

## Automated verification (2026-10-15)

- `./scripts/cargo.sh check --locked --all-targets`: pass.
- `./scripts/cargo.sh fmt --all -- --check`: pass after installing the missing official rustfmt component. The included UI source is checked separately with `rustfmt --edition 2024 --check src/guides_ui.rs`.
- `./scripts/cargo.sh test --locked`: **114 passed** (85 library including 11 Guides RawInput UI tests, 7 Guides persistence/history integration tests, 22 existing workflows; zero failures).
- `git diff --check`: pass. Pre-existing source changes retained; rustfmt normalized existing Rust formatting without intentionally changing other feature behavior.
- RawInput covers guide construction/cancel, exact circles/arcs, tabs, drag/undo, swept eraser, independent snapping, isolated intersections, modal validation/Apply/Cancel, numeric wall-distance Apply and keyboard input.
- Readable source: `src/guides.rs:47` GuideSettings, `:93` GuidePrimitive, `:503` snap_candidates; SHA-256 `d3e46622cf2fbb7487dfb3e146362a26f641a81010acd14e1bafb2a81012bd36`. `src/distance_guides.rs:38` measurements, `:221` apply_distance; SHA-256 `2219e3ec03db097cb36dc4ff6e5f70969deb85099acce32166e8529bfdd53212`. `src/guides_ui.rs:46` guides_menu, `:738` guide_canvas; SHA-256 `2308a8da561baebf12d86231b720431181cd9dfe19b2d767a3cb6639900ddfd2`. `src/model.rs:159` stores guide settings and fixed guides with serde defaults.

Bundle freshness and desktop smoke are recorded below after packaging.

## Packaged artifact / desktop attempt

`./scripts/package-macos.sh` passed and produced `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`. `dwarfdump --uuid` reports the identical arm64 UUID `179E1B19-1215-33AD-8C9A-5E141F72A195` for `target/release/openarchi` and the bundle executable. `codesign --verify --deep --strict dist/OpenArchi.app` passed (exit 0).

CUA `getApp` using the exact packaged app path attempted launch, but returned `Computer Use server error -10005: cgWindowNotFound`. A subsequent read-only process check found PID 35996 running the exact `dist/OpenArchi.app/Contents/MacOS/openarchi` path. This proves the current packaged process launched; it does not prove a working window or successful desktop interaction. **Desktop menu/drawing smoke remains unverified.** No manual clicks were performed in OpenArchi; the 11 pointer/modal tests are egui RawInput tests, not desktop tests. Archicad's menu/settings observation was completed earlier using CUA and the settings dialog was cancelled. Only the REA binding opened by this task was closed.

Final review retained the pre-existing dirty workspace, including stair/polyline/cursor changes and existing docs. No commit, tag, push or publication was performed.
