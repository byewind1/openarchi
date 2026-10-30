# Default node edit, pencil feedback and hover activation — 2026-10-29

Final app `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`; release/bundle arm64 UUID **0A21A3A6-9135-3AD1-99D0-8AD06964E908**, strict/deep signature passed. Forced receipt `target/ac29-workflow/runs/20261028T195222Z-83063bfc/report.json`: **402 Rust tests + 15 tooling tests passed**, formatting/includes, all-target check, locale checks, diff and fresh packaging passed. Final `bundle` freshness matches this successful source receipt. One existing performance test remains ignored.

## Behavior

Clicking the middle reference segment of an already selected wall now immediately activates Insert node and its preview. The initiating click cannot commit. Original finite reference path within 4 screen points → solid pencil; outside → hollow pencil, including when the pointer remains over the wall body. Uses original model geometry and current plan/3D projection, not the preview path. Hovering another palette icon activates and highlights it; moving back to the canvas continues that command. Same-command reentry and idle hover preserve staged points/numeric parameters. P/Shift+P remains available. Status follows the current command/stage. No geometry operation changed.

The existing wall palette is the supported object scope. Endpoint handles retain endpoint editing. A fresh press-drag without first completing the baseline click retains the prior direct movement shortcut; after baseline click, move/held-drag belongs to pending node editing. Confirmation still uses click/Enter/Apply; Escape/Cancel discards the draft.

## Automated evidence

Three new regressions cover both languages, actual default entry, original-line solid→hollow→solid under plan rotation, all ten hover-only transitions, actual highlight rectangle paint (accounting for fade opacity), no model/history mutation, hover-selected curve commit/undo, active rotation hover preserving two accepted points, curved-path versus chord feedback, projected 3D solid/hollow and no initiating-click commit. Existing default-entry tests and creation→selection→Drag scenario updated to select the desired palette command. Full suite includes prior geometry/persistence/undo tests.

## Final packaged native evidence

Isolated HOME `target/wall-pet-cursor-smoke-home`, actual verification PID **67172** / window **42340**; restart PID **74683** / window **42357**. Initial setup PID65521 also closed. Existing user PID46331 had HOME `/Users/ren` and was not targeted. Test HOME restored `en`, test processes closed.

Fixture: [one rotated wall](fixture.json), (-3,-1)→(3,1), 0.3 m thickness, 3 m height. Opened through native File > Open JSON and the app's Open/replace dialog from an isolated copy. Initial recovery seeding did not show a prompt and the file was absent on inspection; exact cause unknown. Readable-source import probe confirmed the fixture validates. An AX text replacement did not change the dialog path; copying the fixture to its existing isolated Documents path allowed normal opening. These setup attempts are not feature acceptance claims.

| Claim | Result / evidence |
|---|---|
| Baseline click directly starts node mode | pass; select wall, click middle (895,562), no icon click; [solid EN](solid-en.png), [AX](default-node-en-ax.txt) |
| Original line versus outside pencil | pass; solid on original line, move to (820,430) shows hollow pencil and node preview; [hollow EN](hollow-en.png) |
| Hover alone activates and highlights Curve | pass; pointer move to (979,597), no button down/up; tracker becomes Curve edge, second icon highlighted, arc preview shown; [screen](hover-curve-en.png), [AX](hover-curve-en-ax.txt) |
| Hover alone activates Rotate | pass; pointer move to (979,638), tracker requests rotation center, second bottom icon highlighted; [EN](hover-rotate-en.png), [ZH](hover-rotate-zh.png) |
| Return to Insert and baseline | pass; hover first icon, move to original line, solid pencil restored; [ZH](solid-zh.png); moving outside gives [hollow ZH](hollow-zh.png) |
| Confirm/undo/redo | pass; canvas click makes two connected walls with unchanged original endpoints; [result JSON](node-result.json), [screen](node-result-zh.png). Native Undo restores one wall; Redo restores exact parsed JSON |
| Projected 3D default and cursor | pass; selected top reference edge click (904,457) starts Insert node without commit; [solid](model-solid.png), move outside gives [hollow](model-hollow.png), Cancel returns to original |
| Language | native View > Language EN→ZH→EN; node/rotation tracker and palette, title/menu/toolbar/Info Box/toolbox/tabs/counts/status/Quick Options bar inspected. Switching presentation kept model counts/selection; parsed result JSON identical after switch and redo |
| Restart | Chinese first-frame title 未命名 observed, [screen](chinese-restart.png); isolated preference restored English |

Exact model/history/dirty invariance during all hover transitions is automated; native observation establishes visible counts/title and captured model output, not full introspection of the undo stack. Other eight commands' hover activation is automated, not individually native-captured in this follow-up. Both-language 3D-specific pencil screenshots, floating Quick Options, and unrelated dialogs not rechecked. Title-bar language selector remains absent under prior native-title product choice. No claim of complete application localization or new AC29 runtime equivalence.

## Experience candidates for developer confirmation

- Treat hover activation as a command transition on entering an icon, not an unconditional reset every frame. Reentering the currently active command must preserve accepted points and numeric locks; keyboard cycling while the pointer rests over an icon must not be immediately reversed.
- Classify edit cursors against the original finite geometry in screen space. Preview geometry, world-distance tolerances, and the element's filled body answer different questions and can give misleading solid-pencil feedback.
- When an existing click begins a new pending operation in the same frame, explicitly prevent that click from also confirming it. Prime floating-area layout on the entry frame so its temporary sizing rectangle cannot swallow the next canvas event.

Candidates only; AGENTS.md reusable rules were not changed.
