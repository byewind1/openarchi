# Edit shortcuts — 2026-10-28

> 2026-10-28 用户纠正：现有快捷键仍进入数值对话框，**不等于完成 AC29 鼠标编辑交互**。后续实现必须查阅 [鼠标交互与过程修饰键契约](pointer-interaction.md)；该记录包含官方帮助、用户描述差异及未决项。完整标准配置见 [171 条快捷键](standard-shortcuts-full-29.md)。


## Contract and evidence

Target: installed Archicad 29 INT 5101, identity reused by `ac29-workflow.py context edit`.
The AC29 `Standard Shortcuts 29` XML is readable shipped configuration, not proprietary implementation code. Only factual command/key mappings are retained in [standard-shortcuts-29.json](standard-shortcuts-29.json), with source path/hash and context identifiers.
CUA captured the current Edit hierarchy, but its AX tree did not expose shortcut characters and its screenshot was unavailable. The user's active custom scheme is therefore **unknown**. This implementation matches the installed standard global + non-text scheme, not a guessed active custom scheme.

| Command | macOS standard |
|---|---|
| Undo / Redo / Repeat | ⌘Z / ⌥⌘Z / ⌘B |
| Cut / Copy / Paste / Select All / Find & Select | ⌘X / ⌘C / ⌘V / ⌘A / ⌘F |
| Group / Ungroup / Suspend Groups | ⌘G / ⌥⌘G / ⌥G |
| Drag / Rotate / Mirror | ⌘D / ⌘E / ⌘M |
| Drag / Rotate / Mirror a Copy | ⌥⌘D / ⌥⌘E (also ⌥⌘R) / ⌥⌘M |
| Quick Rotate CW / CCW | ⇧⌘R / ⌘R |
| Elevate / Multiply / Resize / Adjust / Explode | ⌘9 / ⌘U / ⌘K / ⌘− / ⌘= |
| Tool Settings / Edit Selection Set | ⌘T / ⌥⌘T (latter backend still unavailable) |
| Pick Up / Inject Parameters | ⌥C / ⌥⌘C |

The existing ⇧⌘Z redo alias is retained and explicitly marked as OpenArchi compatibility.
Delete/Backspace retains existing deletion/drawing-node cancellation behavior.
No bindings are invented for commands absent from this standard configuration (including Autogroup, Align/Distribute and Multiple Copies).

## Behavior

One table supplies dispatch and native/egui menu hints. Exact modifier matching prevents a copy command from invoking its base transform, or disabled ⌥⌘T from opening Tool Settings. Press dispatches once; auto-repeat does not multiply operations. Text inputs keep keyboard editing; modals gate model shortcuts. ⌘D opens the existing numeric Drag workflow and never performs legacy Duplicate. Escape cancels the numeric operation; OK commits one undo step. Existing geometric limitations in coverage.md remain unchanged.

⌘A uses the same command path as the menu: active marquee scope when applicable; otherwise visible current-story elements. No project/undo mutation for selection. On non-macOS platforms Command is adapted to the platform Ctrl modifier; only macOS evidence is claimed.

No AppKit key-equivalent is installed for these model commands, so it cannot steal text-widget clipboard/history shortcuts before egui sees them. The menu displays the actual mapping. This follows the existing native-menu architecture.

Acceptance: evidence-table conformance; each supported binding equals menu dispatch; base/copy modifiers and rotation directions distinct; no-selection/locked cases; no repeats; text focus and modal isolation; ordinary and marquee Select All; numeric apply/cancel + undo/redo; EN/ZH labels; full tests and fresh app packaging. Desktop verification must be new runtime evidence, otherwise unknown.

## Toolkit input evidence and tests

Readable source: local `egui-winit 0.36.2/src/lib.rs` `on_keyboard_input` and `is_copy_command`/`is_cut_command`/`is_paste_command` replace C/X/V key-downs with `Event::Copy/Cut/Paste` (including Cmd+Alt+C → Copy). `src/app/edit_shortcuts.rs` handles those events and recovers the parameter-injection modifier. Copy/Cut emits an OS clipboard text marker `OpenArchi elements`; element values remain in the application clipboard. The marker permits egui-winit to emit Paste even when the OS clipboard was previously empty. External BIM clipboard interoperability is not implemented.

RawInput regressions cover every table binding against the same menu dispatch, command variants, selection scope, dialog apply/cancel, both redo chords, native-style clipboard events, parameter injection, text focus, modal gating and key-repeat suppression. Test helper correction: egui derives repeat from held-key state; separate synthetic strokes must release the previous key. Clipboard semantic events themselves do not carry a repeat flag.

Reusable lesson candidate (awaiting confirmation): test the toolkit's clipboard events as well as Key events; Cmd+Alt+C can arrive as Copy, and testing only Key::C can falsely certify parameter-injection shortcuts. No AGENTS.md update made.


## Shortcut delivery verification — 2026-10-28

- Fresh `./scripts/cargo.sh test --all-targets`: 332 passed, one existing performance test ignored.
- Fresh `python3 scripts/ac29-workflow.py verify --force`: formatting (including included files), cargo check, localization/duplicate-key checker, 332 Rust tests, 15 tooling tests, diff checks and macOS packaging all passed.
- Final app: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/dist/OpenArchi.app`. arm64 UUID `A89C6B50-EEF7-350B-8A11-0902CCA10313` matches release; deep strict ad-hoc signature passed.
- Receipt: `/Users/ren/MAC工作/工作/code/开源项目/openarchi/target/ac29-workflow/runs/20261028T081700Z-d1fb29c6/report.json`.
- New isolated-English launch: `target/edit-shortcuts-smoke-home`, PID 67397. CUA binding to the exact new bundle path returned `Computer Use server error -10005: cgWindowNotFound`. No native keyboard action, screenshot, EN→ZH→EN or restart-in-Chinese was verified. Headless bilingual menu-label, modifier, text-focus, modal and clipboard tests are not native acceptance.
- Existing title-bar selector is absent from the earlier native-title implementation. View > Language remains the available entry; this task did not restore a title-bar picker. User preferences left untouched through isolated HOME.
- Standard configuration: 29 bindings for 28 Edit commands. One command (Edit Selection Set) remains disabled with its reserved chord. OpenArchi additionally preserves ⇧⌘Z. Unbound commands have no invented shortcut; backend limitations in coverage.md remain.
- No commit or push. Existing unrelated changes preserved.
