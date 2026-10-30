# Edit menu coverage — 2026-10-28

> 2026-10-28 用户纠正：现有快捷键仍进入数值对话框，**不等于完成 AC29 鼠标编辑交互**。后续实现必须查阅 [鼠标交互与过程修饰键契约](pointer-interaction.md)；该记录包含官方帮助、用户描述差异及未决项。完整标准配置见 [171 条快捷键](standard-shortcuts-full-29.md)。


Status: substantial implemented subset; **not complete AC29 Edit equivalence**.
All original menu entries are represented by the shared catalog in `src/app/edit_menu.rs`.
Disabled entries never claim to apply a change. Source inspection is evidence for routing; tests below establish only bounded cases.

| Commands | Implementation / limits | Verification |
|---|---|---|
| Undo / Redo, Cut / Copy / Paste, Delete, Select All | Existing editor paths; copy now retains name, layer, renovation, design option and wall settings, remaps included hosts/groups. Paste uses a fixed 0.5 m XY offset and application clipboard; no external AC clipboard compatibility. | hosted-copy, lock, delta and persistence regressions |
| Repeat Last Command | Repeats last new typed EditCommand on current selection; does not repeat every legacy command. | source routing; desktop unknown |
| Find & Select | Case-insensitive name substring and type; visible current-story elements. No AC criteria-set editor. | dialog source; desktop unknown |
| Group / Ungroup / Suspend Groups / Autogroup | Nested membership sets, expanded selections, one-level ungroup or suspended individual detachment. Autogroup supports newly created multi-segment walls and exploded paths. | nesting, suspension, history, JSON regressions |
| Lock / Unlock / Unlock All | Element locks; existing layer locks remain independent. Atomic edit preflight plus legacy commit guard. | locked batch/delete rollback |
| Display Order | Reorders persistent element vector with stable one-step/front/back/default-ID ordering. Wall rendering remains batched; cross-type visual ordering is **not AC-equivalent**. | exact order undo; visual overlap unknown |
| Drag / Rotate / Mirror + copy and multiple copy variants / Multiply | Numeric displacement, pivot, angle, copies. One transaction, host and group remapping, arc scale/handedness. Multiply is a linear/rotation step, not the full AC matrix/path/distribution dialog. Mirror axis is vertical through origin X. | copies, arc, invalid transforms, dialog OK/Cancel |
| Quick Rotate CW / CCW | ±90 degrees about selection bounding-box center. | routes through tested transform; desktop unknown |
| Elevate | Walls and openings; changes bottom/top offsets and hosted sill together. Slabs/stairs unsupported and rejected atomically. | explicit wall/opening measured regression |
| Edit Elements by Stories / Relink Home Story | Moves current selection to target story. Relink preserves absolute wall/opening elevation. No full story copy/delete/element-type matrix. | validated source; desktop unknown |
| Horizontal/Vertical Align | Aggregate bounding-box edge/center; hosted openings follow wall. | measured alignment regression |
| Along X / Y / XY | Equally spaced bounding-box centers, requires ≥3 independent elements. | measured X regression; Y/XY runtime unknown |
| Resize | Positive plan scale about numeric pivot; arc radii and opening offsets/width scale. Wall height/thickness unchanged. Slabs reject non-quarter rotation instead of replacing geometry with AABB. Not the full AC resize-options dialog. | radius/handedness, slab rejection, validation |
| Split / Adjust | Numeric cutting line; straight walls and two-node straight polylines. Walls with hosted openings rejected. Split retains both pieces; Adjust extends/trims nearest endpoint. | split/unify regression; Adjust full workflow unknown |
| Fillet/Chamfer | Numeric interior node index and radius/distance on straight-adjacent polyline edges. Exact tangent arc; rejects invalid radius and existing adjacent arcs. | non-right-angle tangent positions, radius rollback |
| Intersect | Two straight walls/lines; nearest endpoints extended/trimmed to common intersection; parallel/hosted cases rejected. | measured endpoints + undo |
| Offset | Directed straight wall or exact polyline/arc offset via shared geometry kernel. | existing arc offset tests; desktop unknown |
| Explode into Current View | Selected polyline edges or rectangular footprints become independent polylines; destructive with undo. Does not reproduce full AC projected 3D/stair detail. Hosted opening selection rejected. | source; desktop unknown |
| Unify | Connected straight polyline chains sharing story/layer/material. No polygon union. | split → unify + undo |
| Linework Consolidation | Exact duplicate polyline shapes with matching story/layer/material only. No overlap splitting or line-type consolidation. | source; desktop unknown |
| Merge Walls | Two collinear, touching, same-direction walls with matching properties and no openings. Keeps first id; one undo. | valid merge, differing material rejection, undo |
| Tool Settings / Pick Up / Inject / Transfer Settings | Existing workflows retained. | existing suite |
| Modify Wall Reference Line / Invert Wall Direction | Numeric body offset with reference-line relocation; inversion reverses endpoints, body-offset sign and hosted center-distance. | invert preserves measured footprint |

## Still unavailable

Search & Replace Text; Special Align; Special Distribute; Trim; Stretch menu workflow;
Fill Consolidation; Split into Single-plane Roofs; Adjust Elements to Slabs;
Merge Beams; Merge Columns; Modify Slab Reference Plane; Edit Selection Set.

Dependencies include missing text/fill/beam/column/roof model types, nonrectangular slab geometry,
and interaction contracts not yet implemented. Existing marquee stretching remains available
through its prior workflow, but is not incorrectly presented as a completed Edit > Stretch.
Graphical pointer-driven AC interactions and all-element compatibility remain separate gaps.

## Reusable lesson candidates (not confirmed, not added to AGENTS.md)

- Native and egui menus should share a command catalog and capability checks, so one cannot silently retain preview-only routing after the other gains a backend.
- Copy full element values before remapping ids/hosts; rebuilding from shape/material silently drops layer and wall parameters.
- A rectangular-only model must reject arbitrary rotation; rotating corners then taking their AABB changes the element.
- Wall elevation needs bottom offset, top offset and hosted opening sill changes together; testing only the wall baseline misses altered heights and invalid openings.

## Final validation — 2026-10-28

- `./scripts/cargo.sh test --all-targets`: 324 Rust tests passed; one existing performance test ignored.
- `python3 scripts/ac29-workflow.py verify --force`: fresh execution, all format/include-format checks, cargo check, localization checker (duplicate keys included), 324 Rust tests, 15 tooling tests, diff check, macOS release/package passed.
- App: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`.
- Release and bundle arm64 UUID: `A93D50AE-DC6E-34FB-8468-FE928B4EBB94`; `codesign --verify --deep --strict` passed (ad-hoc, not notarized).
- Desktop launch used isolated `target/edit-menu-smoke-home`, seeded English. The first packaged process PID 13190 was confirmed with that HOME. CUA getApp(bundle id) and then getApp(exact app path) both failed with `Computer Use server error -10005: cgWindowNotFound`. App inventory reported OpenArchi running. This is launch evidence only, not manual editing verification. The owned disposable process was closed and the final toggle-indicator build relaunched.
- EN/ZH numeric Drag dialog labels, pointer Cancel/OK, Escape, model/history stability across language rendering are covered by egui RawInput tests. Actual EN→ZH→EN switching, View > Language, restart-in-Chinese, main-window surfaces, all other edit dialogs and model manipulation on desktop remain **unknown** due to the current window-control blocker.
- A title-bar language selector was already removed in the existing native-menu/title work; current source exposes View > Language. The requested independent title-selector test therefore cannot pass and is recorded as absent, not silently replaced. No user preference was changed by these isolated launches.
- No commit/push. The pre-existing dirty worktree was preserved; rustfmt formatted touched source.

Source evidence (readable source, final SHA-256):
- `src/editing.rs`: `8618c17fbdca31b757f731a363af9e2d98789a87e0d25628b235c79f2f2e254d`
- `src/app/edit_menu.rs`: `97d299ec7be6eec9e3c75151c9e198cc0fe3890bda89be1e7521ea3c80d3efe1`
- `src/editor.rs`: `0c24df02f4d58952eb72664c8537a3791b84afa2d55d3643eb4e7079c8240092`
- `src/app/native_menu.rs`: `e0f26c98aa147edb91cdf8d6522ad1d2bf9ec1d886bdb7b552d2e85e6fc479b3`
- `src/command.rs`: `511d75b33095af189aac3a7447fa3cfc71575186b7c51af2d97e2cc1161ddfe5`


## Shortcut follow-up — 2026-10-28

The earlier shortcut gap is addressed for existing command backends: see [shortcut mapping and validation](shortcuts.md). Final app UUID is now `A89C6B50-EEF7-350B-8A11-0902CCA10313`, with 332 Rust and 15 tooling tests passed. This supersedes the earlier bundle receipt above, not the command-geometry limitations. Native desktop verification remains blocked by the current `cgWindowNotFound` error.
