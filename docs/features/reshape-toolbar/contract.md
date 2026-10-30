# Standard toolbar / Reshape — 2026-10-29

Target: AC29 INT 5101; SHA-256 `d86af90a1fc90f0a9ef2c8565300179acc9735dda8fc3e0fe07611e8aa8b5505`.
User asks for screenshot commands in standard toolbar and Edit > Reshape, with object applicability and real interaction.

## Evidence and command map

Left to right (screenshot visual identification corroborated by official toolbar descriptions):

| Command | AC29 objects and workflow | Evidence |
|---|---|---|
| Trim | Walls, beams, lines, circles/arcs, polylines, splines. No selection required; scissors click removes interval between nearest intersections/endpoints. Cmd-click shortcut on Mac; wall/beam surfaces also in 3D. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-89.htm); runtime menu entry and instruction, below |
| Magic Wand | Activate destination tool, Space or command, hover preview then click edge/node to trace chain, or interior to trace enclosed region; selection restricts source search. New independent elements use tool defaults. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-53.htm) |
| Adjust | Selected walls, beams, railings, MEP routes, arcs, lines; draw boundary or click edge/arc. Extend/shorten endpoints. Slab edge uses separate slab-adjust workflow. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-86.htm) |
| Intersect | Two lines/polylines, arcs, walls or beams, visible on same story; immediate nearest-endpoint meeting. Groups require suspension. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-76.htm) |
| Fillet/Chamfer | Two straight lines/polylines or walls, or polygon corners. Radius; chamfer is chord of corresponding arc. Polygon menu applies all corners, pet palette may apply one. Radius is capped at available edge length. Groups require suspension. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-88.htm) |
| Resize | Selected elements; numeric ratio then click fixed point, or graphical ratio/reference vector. Optional wall/column/beam thickness scaling; groups require suspension. | [AC29 help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-84.htm); runtime dialog below |
| Trim Elements to Roof/Shell | Model elements trimmed by roof/shell bodies; automatic multi-selection or manually choose trimmer then retained side. Associative connection requires roof/shell model, not scissors geometry. | [automatic](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-92.htm), [manual](https://help.graphisoft.com/AC/29/INT/_AC29_Help/040_ElementsVB/040_ElementsVB-95.htm) |

Magic Wand's AC menu is Design > Outline Polygon with Magic Wand; Roof/Shell trim is Design > Connect. Adding aliases to Reshape is an explicit user-requested OpenArchi difference.

Runtime: existing GDL 测试 2.pln, no initial selection, no model mutation. CUA Edit > Reshape > Trim changed status to “Click an element to remove the part of it beyond the endpoint or intersection point.” REA evidence `ev_28cd417748c4a58e31a7c368ed86ad3d115a8f031dd8cbafbefa4a2b1000a6bd`, [full record](evidence/ac29-trim-entry.json). Escape exited. Select All selected one wall; Resize dialog displayed ratio 1/1/100%, Define graphically and thickness checked. Those are observed persisted UI values, not proven factory defaults. Model effects are documentation, not runtime-measured equivalence. No synthetic modal pixel clicks.

## Implementation contract / reversible assumptions

Original Rust geometry and vector icons; one command dispatcher for toolbar, native and egui menus. Plan pointer modes retain selection and revision, preview private copies, Escape cancels without history; successful validated candidate commits once. Invalid, locked, grouped or unsupported geometry is rejected without changing the model. Geometry lives outside UI; plan and 3D derive from saved elements. Current model: straight walls, rectangular slabs, stairs, polyline arcs, hosted openings. Roof/shell type unavailable pending user scope decision. Do not approximate arbitrary polygon slabs with bounding rectangles. State support limits explicitly in UI and tests.

Acceptance: each enabled toolbar/menu command; object eligibility; real pointer clicks; preview/cancel; exact geometry; invalid and locked rollback; undo/redo and JSON roundtrip; both languages and entry points/restart on final packaged app. Final validation and remaining unknowns to be appended after execution.

## Delivered subset and limits — 2026-10-29

- `src/reshape.rs`: `ReshapeCommand`, `candidate`, `Editor::reshape`, `Editor::magic_wand`, `trace`; original validated candidate operations. `src/app/reshape_ui.rs`: shared eligibility, tooltips, pointer session, canvas preview and dialogs; standard toolbar and native/egui Edit menus share dispatch. Reshape aliases include Magic Wand and Roof/Shell Trim. Original vector icons are drawn in `paint_tool_icon`.
- Trim: straight walls without hosted openings and straight polylines; click removes interval bounded by actual visible source segments, retaining metadata and separate remainders. Polygon/slab straight edges may act as boundaries. No selection prerequisite. Curved sources and 3D surfaces remain unsupported. Cmd-click and repeated mode are implemented by the correction below.
- Magic Wand: Wall / Polyline / rectangular Slab destinations, current tool defaults, straight wall/polyline/slab sources, current story and visible layers; selection restricts source set. Edge click traces source and unambiguous connected endpoint chains; interior click enumerates the smallest bounded face after splitting segment crossings. Branches do not choose an arbitrary continuation. Curves, holes, trace-reference and remote-story sources, polygon add/subtract, approximation settings remain unsupported. Source limit for region tracing: 512 segments. Nonrectangular slab results are rejected, never converted into bounding boxes.
- Adjust: selected straight walls without openings and straight polylines; click a source edge or two free boundary points. Preview and one commit; nearest eligible endpoint is extended/trimmed against the infinite straight boundary. Slab-edge vertical adjustment and curved boundaries remain unsupported.
- Intersect: exactly two straight walls or two-node straight lines, immediate operation through existing validated geometry; group suspension and locks enforced. Curved/multi-node end intersection remains unsupported.
- Fillet/Chamfer: two straight lines (trim/extend plus independent connector arc/line), all corners of one straight polyline, or two straight walls for **Chamfer only**. Wall arc body does not exist, so Fillet is explicitly unavailable for walls. Chamfer uses corresponding arc tangent points, not a generic offset distance. All-corner radii share adjacent edges conservatively and preserve the model's minimum segment length. Mixed wall/line input rejected; metadata of connector uses first element. Unequal source attributes, GDL objects, slabs/polygon model corners and exact AC radius clipping are not claimed equivalent.
- Resize: numeric scale then fixed point; or fixed point → reference-distance point → new-distance point. Optional wall thickness/height scaling. Uses existing transforms for walls, slabs, stairs, polylines and hosted-opening remapping. Unsupported/locked/invalid complete candidates roll back. Standalone opening selection is disabled. Groups require suspension. No 3D pointer workflow or library/text/marker scaling options.
- Roof/Shell trim: visible in both requested places, disabled with explicit model-support tooltip. User was asked about extending scope; no answer received during implementation. **No roof/shell geometry or associative trim was implemented.** This is not approval to build those types.
- Units: metres and scale factor; default graphical Resize enabled, factor 1, thickness enabled; fillet radius 0.25 m. OpenArchi choices, not proven AC factory values. Escape/cancel changes no project/history. Successful new reshape operations clear stale Repeat Last Command instead of incorrectly repeating a previous unrelated operation.

## Verification

Fresh `python3 scripts/ac29-workflow.py verify --force`: passed at 2026-10-28T17:51:59Z (2026-10-29 Asia/Shanghai). Formatting including included files, `cargo check --locked --all-targets`, locale checks, `cargo test --locked --all-targets`: **364 passed**; Python tooling **15 passed**; diff checks; fresh macOS packaging.

Bundle: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`.
Release and bundle arm64 UUID: `BD0A3CB4-A003-3F81-AA7C-CB1D4EC3A237`; `codesign --verify --deep --strict` passed. Source/artifact receipt: `target/ac29-workflow/runs/20261028T175117Z-2836eec4/report.json`. This supersedes the earlier 363-test / E37C9E59 bundle.

14 added regressions cover exact trim remainder coordinates and metadata, preview isolation, invalid/locked/group rejection, parallel no-op, tangent/chord geometry, oversized polygon radius, closed-face and connected-edge tracing, selection restriction, nonrectangle rejection, defaults and atomic magic creation, fixed-point resize, serialization, undo/redo, actual workspace toolbar clicks and plan pointer events, graphical three-click resize and both-language rendering. Existing checks also passed. Tests named in `src/reshape.rs` and `src/app/ui_tests.rs` with `reshape_` prefix.

### Final packaged desktop evidence

Final instance PID 2944 / CGWindow 41654; isolated HOME `target/reshape-smoke-home`. Actual first-frame title `未命名` after retaining Chinese preference, then View > Language English → 简体中文 → English using native AX menu actions. Final isolated preference `en`; real user's HOME preference untouched. Native menu labels captured in [English](evidence/final-en-menu.txt) and [Chinese](evidence/final-zh-menu.txt).

Screenshots: [Chinese first frame](evidence/final-zh-restart.png), [English main window](evidence/final-en.png), [Chinese main window](evidence/final-zh.png), [Resize English](evidence/final-resize-en.png), [Resize Chinese](evidence/final-resize-zh.png). These cover actual final bundle title, standard toolbar/icons, Info Box, toolbox, tabs, counts, status, Quick Options bar, navigator, and Resize parameters. User/project story names remain data, not translated labels. Native Select All selected 11 current-story elements; Resize opened via Edit > Reshape. Language switch retained selection count, visible geometry and untitled clean title; exact history/model invariance is covered by automated tests, not extracted from the desktop process.

**Desktop unknown/blocked:** CUA `cgWindowNotFound` despite a live window; AX introspection exposed native menus but not egui controls. Bounded CG mouse/key input attempts did not activate recovery/dialog canvas controls. No successful native canvas completion, cancel, undo/redo, save/reopen or Fillet dialog claimed. The native-title configuration has no visible title-bar language selector; that entry point is unverified/unavailable, not a localization pass. Quick Options floating palette was not checked. Runtime-only comparison to AC trim geometry remains unknown. Full localization and full AC equivalence are **not** claimed.

The temporary initial recovery fixture had incorrect JSON shape tagging and was corrected before retry; it was never applied to a user project. Test app instances were terminated without committed edits. Only this investigation's REA target was closed; Archicad selection restored to empty, no model edit/save.

## Candidate reusable lessons (awaiting developer confirmation; not added to AGENTS.md)

- Native menu refresh signatures must include active tool and pointer-session state, not just selection and model revision; otherwise menu eligibility can disagree with the toolbar.
- All-corner fillets must allocate shared edge length and preserve minimum residual segment length; independently clamping each corner to a full edge can produce invalid polygons.
- Toolbar command tests should click the actual workspace widget and complete its pointer state machine; geometry-only tests cannot detect dispatch, selection or premature-commit errors.

## Historical 2026-10-29 follow-up — Ctrl (superseded by user correction below)

User explicitly requests **Ctrl**, including this macOS build; this product instruction takes precedence over the previously cited AC help's Cmd-on-Mac convention. Holding physical Ctrl alone over the plan temporarily routes hover/click to scissors Trim. Release exits; no toolbar/menu activation required. Existing menu Trim remains a persistent one-shot command. No mutation on key press, hover or release; each successful Ctrl-click is one undoable trim. Preserve active tool, existing wall/polyline draft and selection on mere hover/release. Do not invoke the explicit command's draft-cancel path. Ctrl+Alt/Cmd/Shift chords, text fields and other active editing modes retain their routing. A mouse press owned by Ctrl Trim must not become a drawing/selection click if Ctrl is released before mouse-up. Same model/object restrictions as existing Trim.

Implementation: `trim_modifier_active` and per-frame `ReshapeSession::new` route Ctrl to the existing validated Trim operation without calling `start_reshape`. A separate mouse-gesture ownership bit consumes an unfinished Ctrl-click after modifier release. Direct drag and parameter-transfer routing defer to the temporary Trim mode. No persistent reshape session is stored on hover or when outside the canvas. The earlier unsupported **Cmd-click** statement remains about Cmd; physical **Ctrl-click is now implemented**, as explicitly requested.

Candidate reusable lesson (unconfirmed): temporary modifier tools should construct transient state without invoking normal command-start cancellation; retain mouse-gesture ownership through release so a change of modifiers cannot turn a trim click into a drawing click. Not written to AGENTS.md.

### Ctrl follow-up validation — final package

Forced verification receipt `target/ac29-workflow/runs/20261028T180350Z-ab1bd890/report.json`: **369 Rust tests passed, 15 tooling tests passed**, formatting, all-target check, localization, diff and packaging passed. Five new `trim_ctrl` full-workspace egui regressions cover hold/release and draft preservation, repeat clicks and exact undo/redo, modifier-before-mouse release, outside-canvas/chords/explicit mode, locked-target rollback, and both tooltip translations.

Final release/bundle arm64 UUID **E8A6B374-EEA4-3641-889F-03610EBD897D**, strict deep codesign passed; supersedes BD0A3CB4 above. App: `dist/OpenArchi.app`.

Native final-package smoke used isolated HOME `target/trim-ctrl-smoke-home`, PID 11406/window 41719. View > Language AXPress switched English → 简体中文 → English; screenshots [English](evidence/ctrl-final-en.png), [Chinese](evidence/ctrl-final-zh.png), [English restored](evidence/ctrl-final-en-restored.png) show title, toolbar, Info Box, toolbox, tabs, counts, status and Quick Options bar. Default project remained visibly 12 elements, selection 0, clean title. Set Chinese again and restarted as PID 36492/window 41738: CGWindow title `未命名` confirmed first-frame persistence ([capture](evidence/ctrl-final-zh-restart.png)). Restored isolated preference to English and closed test instances. User's existing PID 38825 was not edited or closed.

Current desktop limits: CUA app-path binding selects the user's older instance; explicit window binding is unsupported. Thus actual native Ctrl-click/tooltip hover remains **unknown**, with semantics verified through egui events only. Title-bar language selector is absent by existing product configuration; floating Quick Options, other reshape dialogs and exact desktop model/history invariance were not rechecked on this package. No complete-localization or AC-equivalence claim.

## 2026-10-29 correction — Cmd and repeated interval Trim

Latest user instruction supersedes the earlier Ctrl request: physical **Cmd** alone activates temporary Trim on macOS; Ctrl alone must not. Keep each click independently undoable, preserve drawing draft on mere modifier activation/release, and do not steal Cmd keyboard shortcuts. Explicit toolbar/menu Trim remains active after each successful cut until Escape; held Cmd permits successive cuts and release exits the temporary mode.

User-specified geometry: remove the clicked interval, bounded by the nearest intersection on each side or the target endpoint. End intervals stop at the first intersection from that end, never skip to a farther intersection; recalculate from current geometry after every cut. Clicking between two intersections deletes only that middle interval and retains both remainders. Existing boundary representation: wall reference line, straight polyline segments, rectangular slab perimeter, visible on the current story. Do not mix wall reference and footprint into multiple artificial stops. Cuts exactly at a new endpoint still count as real boundaries: a final remaining interval between endpoint boundaries can be deleted. With no intersecting boundary at all, retain the current explicit error (reversible assumption; user did not specify isolated-object deletion).

Evidence: user correction and [AC29 public help](https://help.graphisoft.com/AC/29/INT/_AC29_Help/030_Interaction/030_Interaction-89.htm), reread this turn: Cmd on Mac and clicked part between nearest intersections. Repeated explicit mode and endpoint-only deletion are implementation choices matching the user's description, not newly measured AC runtime. Curves/3D/hosted walls remain outside this subset. Acceptance: old Ctrl disabled, Cmd temporary and chord-safe; both explicit/held modes repeated; left/right end progression; middle removal; wall baseline and slab edges; exact endpoint-boundary final deletion; no mutation on preview/error; undo/redo/save roundtrip; localized tooltip, fresh app.

### Cmd final verification and native acceptance

Fresh forced verification: **373 Rust tests, 15 tooling tests passed**; fmt/includes, all-target cargo check, locale checks, diff and macOS packaging passed. Receipt `target/ac29-workflow/runs/20261028T181812Z-05046f41/report.json`. Release/bundle arm64 UUID **967D7AFF-53BC-36FE-B4CF-A75C0B0FDF99**; strict/deep codesign passed; final `bundle` source freshness matches successful receipt. App: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`. Supersedes the Ctrl package E8A6B374. Eleven focused trim tests passed; four added tests cover sequential left/right wall trimming against reference line/slab edges/finite lines, endpoint-bounded deletion, repeated actual pointer input in explicit and temporary modes, and Cmd+Z coexistence. Existing five modifier tests now use physical Cmd plus egui's platform `command` bit, and explicitly reject Ctrl.

**Current native smoke passed for this scope**, using final package PID 32074/window 41913, isolated HOME `target/trim-cmd-smoke-home`. User's existing PID 52583 was preserved. Native menus operated through AXPress; canvas input used foreground-guarded CGEvents to this test instance. Earlier postToPid/background failures are historical, not a blocker for this run. No Archicad input this turn.

Fixture [JSON](evidence/cmd-fixture.json): horizontal line (-4,0)→(4,0), three vertical boundaries x=-2,0,2. Native click positions at screen y=550, x=738/842/947/1052; window bounds (204,1,1512,988), screenshot pixels 3024×1976. Main results:

| Claim | Result and evidence |
|---|---|
| Cmd hover and nearest end cut | pass: [held scissors](evidence/cmd-final-held.png), [first cut](evidence/cmd-final-first-cut.png); start moved -4→-2 |
| Continuous Cmd clicks use current intersections | pass: next two clicks without Cmd release moved start -2→0→2; [snapshot](evidence/cmd-final-after-repeat.json), [screen](evidence/cmd-final-repeated-held.png) |
| Last interval removed, cutters preserved | pass: final click removes horizontal element only; [snapshot](evidence/cmd-final-deleted.json) |
| Undo and middle deletion | pass: four native Undo actions restored original; click x≈-1 removed (-2,0), leaving (-4,-2) and (0,4); [snapshot](evidence/cmd-final-middle.json), [screen](evidence/cmd-final-middle.png) |
| Chinese Cmd cut | pass: next click removes (0,2), retaining (2,4); [snapshot](evidence/cmd-final-zh-cut.json) |
| Menu Trim is continuous | pass: Chinese Edit > Reshape > Trim, three unmodified clicks deleted left remainder, next right interval, final right remainder without re-entering command; [snapshot](evidence/cmd-final-zh-menu-repeat.json), [screen](evidence/cmd-final-zh-menu-repeat.png) |
| Escape, Undo/Redo after continuous mode | pass: Escape exited, native Undo and Redo returned the three-boundary result |
| Both tooltip translations | pass: [EN](evidence/cmd-final-tooltip-en.png), [ZH](evidence/cmd-final-tooltip-zh.png), corresponding AX text files |
| Language switching and persistence | pass: same instance EN→ZH→EN; project recovery JSON byte-identical at language change, selection 2 preserved, later Undo/Redo usable; [Chinese middle screen](evidence/cmd-final-middle-zh.png). Chinese restart PID 51563/window 41927 first-frame title `未命名`; [restart](evidence/cmd-final-zh-restart.png). Recovery reopened final 3-element result: [AX](evidence/cmd-final-reopened-ax.txt), [screen](evidence/cmd-final-reopened.png) |

Restored isolated preference to English and terminated only test instances. Checked main title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar in both languages. Title-bar language selector remains absent by prior product decision; floating Quick Options and unrelated dialogs remain unverified. Exact complete undo stack/dirty invariance across language switch is automated, not extracted from the native process; native selection, geometry, count and later undo/redo checked. Wall baseline/slab-edge numeric results and old-Ctrl rejection are automated, not separately repeated on desktop. Persistence smoke used automatic recovery/reopen, not a native Save As dialog. No complete AC geometry equivalence or full localization claim.

Candidate reusable lessons (unconfirmed, not written to AGENTS):
- Repeated trim must recompute intersections on current geometry and retain endpoint intersections as valid boundaries; otherwise the last segment becomes untrimmable.
- Native Cmd simulation in egui tests needs both `mac_cmd` and platform `command` flags; `Modifiers::MAC_CMD` alone omits the latter and cannot exercise Cmd+Z compatibility.
