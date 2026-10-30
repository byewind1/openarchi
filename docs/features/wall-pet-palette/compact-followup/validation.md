# Compact wall palette and release confirmation — 2026-10-29

This follow-up supersedes the large tracker and click-again drag confirmation described in earlier wall-palette validation. Authority: the user's explicit interaction requirements; this is not a new Archicad runtime-equivalence claim.

## Implementation and automated coverage

The original ten vector icons remain in a 148×64-point palette (26-point buttons). The separate tracker uses 12-point type and only distance/angle, or the command's elevation/copy fields. Command heading, operation instructions, Apply, Cancel and Follow pointer controls are removed. The tracker follows near the pointer, avoids the palette, and stays still when approached/focused for editing. Escape cancellation remains.

Dragging a selected wall's middle reference line starts node insertion; releasing on the canvas applies one transaction. An initial simple click starts a pending preview without splitting. While pending, hover switches commands; releases over icons/fields never commit. Staged commands advance once per accepted release. Numeric input and Enter in the same frame apply the newly parsed values. Plan and projected 3D share this path. Body dragging away from the reference line retains generic movement/copy behavior. Model operations and original-reference solid/hollow pencil rules are unchanged.

New UI regressions exercise both languages: exact compact bounds and removed labels, tracker text editing through pointer/keyboard events, same-event angle text + Enter, field release exclusion, preview immutability, single undo transaction, direct selected-baseline release, projected 3D release with unchanged camera, hover switching while holding the mouse, and release over a palette icon. Existing ten-command staged workflows and geometry tests remain active.

## Experience candidates for developer confirmation

- Read a floating numeric editor's changed values before evaluating Enter/commit; committing a previously calculated preview can apply the previous frame's numbers.
- Exclude the actual floating-control rectangles from canvas input, rather than their union bounding box: the empty space between a palette and tracker remains usable canvas.
- A small floating tracker must avoid the editing palette and allow pointer access without chasing the mouse; validate both input focus and the geometry point beneath adjacent empty canvas.

Candidates only; no reusable AGENTS.md rules were changed for this follow-up.

## Final verification and native evidence

Final app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **85D62F1A-641C-3977-B41A-7F73F94C586C**, strict/deep signature passed. Forced receipt `target/ac29-workflow/runs/20261028T201755Z-a93fb169/report.json`: **404 Rust tests (356 library + 48 integration) and 15 tooling tests passed**; one existing performance test ignored. Formatting/includes, all-target check, locale validation, diff and fresh packaging passed. Final `bundle` recheck matches the successful source receipt. Earlier intermediate packages are superseded.

Native final package: isolated HOME `target/wall-pet-compact-home`, PID 7974 / window 42484, bounds (204,1,1512,988), macOS native egui/wgpu. Confirmed process HOME before input; unrelated user PID 78419 untouched. Opened a disposable copy of the prior rotated-wall fixture via the actual Open JSON dialog. AX actions used for menus; foreground-guarded CoreGraphics used for canvas drags and text/keyboard input. Initial view changes needed a settled frame before projected-edge clicks; the recorded final preview captures were recaptured after settling.

| Check | Result and evidence |
|---|---|
| Compact palette, no lower command block | pass; [English plan](preview-en.png), [Chinese plan](preview-zh.png), corresponding AX files |
| Tracker beside pointer, including 3D avoidance | pass; [3D English](model-preview-en.png), [3D Chinese](model-preview-zh.png); when lower-right overlaps palette, tracker moves above pointer |
| Actual text entry and Enter | pass; mouse-click fields, Cmd+A, type `2` then `90`, Return; [values before Enter](numeric-before-enter-en-ax.txt), [result](numeric-after-enter-en.png), [saved JSON](numeric-result.json). Node is original click anchor (~0.0021765,0.0007255) + (0,2), original endpoints retained |
| Held drag and release | pass; plan (895,565)→(820,430), one release creates two segments; [screen](release-result-zh.png), [JSON](release-result.json). 3D selected baseline drag (904,460)→(820,390) also commits; [screen](model-release-zh.png), [AX](model-release-zh-ax.txt) |
| Hover command switch | pass; move only onto Curve icon; [screen](model-hover-curve-en.png), [AX status](model-hover-curve-en-ax.txt), unchanged element count |
| Undo/Redo and language invariance | pass; native undo returns one wall, redo returns two; saved parsed model after redo and EN restoration exactly equals `release-result.json` |
| Language and restart | pass for touched palette/tracker in plan/3D, title/menu/toolbar/Info Box/toolbox/view tabs/counts/status/Quick Options bar inspected via View > Language EN→ZH→EN. Chinese preference persisted and restart PID 11375/window 42493 first frame has Chinese title `未命名`; [screen](chinese-restart.png), [AX](chinese-restart-ax.txt). Isolated preference restored English and both test processes closed |

Native evidence does not introspect every internal history flag; exact preview/model/undo invariants are additionally automated. Title-bar language selector remains absent under the existing native-title product choice; this entry point is unavailable, not claimed tested. Floating Quick Options, unrelated dialogs, and the other commands' elevation/copy tracker fields were not individually native rechecked in this compact-layout follow-up. Existing 3D caption/navigation instruction strings and project/preset names remain as previously displayed; no claim of full application localization. All ten commands remain covered by automated staged-operation tests. No new AC29 observation claimed.

During final QA, a held canvas drag crossing the relocated tracker initially stopped updating. The final implementation preserves canvas ownership until release and makes tracker fields inactive during that gesture; the existing movement UI regression caught this before final packaging. This is an additional proposed reusable lesson, pending developer confirmation.
