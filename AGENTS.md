# OpenArchi agent instructions

These rules apply to all repository work. OpenArchi is an independent, native Rust BIM application; its workflows may learn from observable authorized software behavior, while its source, model, UI and assets remain original.

## Project-specific reusable workflow

For Archicad 29 feature work or continuing an existing reconstruction, read [openarchi-ac29](skills/openarchi-ac29/SKILL.md). Run `python3 scripts/ac29-workflow.py context <feature>` to reuse the installed-target identity and existing evidence before investigating. The skill and tools are versioned in this repository; the global skill entry is only a link to this copy.

`python3 scripts/ac29-workflow.py verify` performs the repository checks and required packaging with source-to-artifact receipts. A `reused` result means an unchanged prior successful run was revalidated, not that tests were rerun. Desktop interaction still needs current runtime evidence. Use `--force` when fresh execution is needed.

## Default: make progress from a short request

- Treat the user's sentence as the requested outcome. Use the established conversation, target version, repository state and prior evidence; do not ask the user to repeat known context.
- Before coding, inspect `git status` and only the relevant source, docs and existing evidence. Preserve unrelated work. Reuse valid REA sessions and Evidence instead of starting over.
- Ask only when an unresolved choice changes a consequential interaction or risks the user's project/data. Bundle the necessary questions into one focused message. Otherwise choose a small, reversible OpenArchi behavior, document it as an assumption, and continue.
- Do the investigation, implementation, checks and required app build before reporting back. Keep progress messages short and factual. Do not present a plan as completed work.

## Product boundaries

- Keep the desktop app native in Rust with egui/eframe and wgpu; do not replace it with a WebView.
- Keep the in-memory model independent. Treat IFC, PLN, GSM, BSM and other formats as explicit adapter boundaries; claim support only after implementation and verification.
- Recreate observable workflows with original code, controls, icons and assets. Do not copy proprietary source, private resources or branded assets.
- Keep model operations typed, validated, undoable and inspectable. UI and AI features use the same model operations.

## Fast feature workflow

### 1. Identify the exact question and evidence

- Name the command/gesture, entry point, selection/view prerequisites and target Archicad version/build from context. Search only what answers that feature.
- Prefer focused runtime observation for interaction semantics, official help for documented behavior, and readable source for source analysis. A label, screenshot, string or static call path alone does not prove the full behavior.
- For Archicad/native REA work, read the installed `reverse-engineer-anything` skill when needed and inspect the connected tool schemas. A REA skill being installed does not mean a target is bound: reuse the active session or bind the authorized target with the supported tool. Do not run setup/readiness checks repeatedly.
- REA can trace evidence; it does not implement OpenArchi. Native UI capture may not support free pointer drags: inspect its current schema and use the available desktop input tool for gestures it cannot perform. Do not claim runtime observation unless it was actually captured.
- Do not force REA onto ordinary OpenArchi source analysis. Never bypass licensing, authentication, permissions, encryption, DRM or safety controls.

### 2. Write a compact behavior contract before substantial code

Record in `docs/features/<feature>/`, `docs/research.md` or an existing focused note:

- user goal and exact interaction sequence, including start state and selection;
- settings, units, defaults, validation, preview, confirmation and cancel;
- relevant geometry/model, plan/3D, selection/snapping, properties, undo/redo and save/restore effects;
- evidence type for each important claim (`runtime observation`, `public documentation`, `readable source`, `static artifact analysis`, `inference` or `unknown`), with source/Evidence ID and limits;
- reversible assumptions, exclusions and a short reproducible acceptance checklist.

Keep facts, inferences and unknowns distinct. For runtime evidence, record target/build, environment, actions and returned Evidence ID. A screenshot or UI tree supports only what it visibly captures. Static reachability does not prove execution. For source analysis, record file and exact symbol/lines; include a hash when practical. Never invent evidence IDs or treat bounded search as proof of absence.

Trace only the needed path: entry → context/selection → parameters → preview → confirm/cancel → model change → views/properties → undo/redo/persistence. Use a minimal disposable project or authorized copy; protect unsaved work. Reprobe only a concrete unresolved question.

### 3. Implement one complete vertical slice

- Keep geometry/model operations outside the UI event layer where practical; derive plan and 3D from the same model state.
- Include tool entry, settings, interaction, validation/errors, undo/redo and persistence where applicable. Prefer one small complete workflow over disconnected controls or placeholders.
- Add focused regression cases for changed geometry and interaction, including explicit expected outcomes. Mark deferred parts in the contract/UI/docs.

### 4. Verify and package what the user will try

- Run checks appropriate to the change using repository scripts in `CONTRIBUTING.md` (normally formatting, `cargo check` and relevant tests). Do not claim checks that were not run or passed.
- For changed dialogs, drawing, shortcuts or selection, exercise egui event handling. For geometry changes, cover validation, persistence and undo semantics as applicable.
- After every source change intended for the user to try, build a fresh `dist/OpenArchi.app` with `./scripts/package-macos.sh` after source checks pass. A release binary or temporary bundle is not the deliverable.
- Verify freshness: `dwarfdump --uuid target/release/openarchi` must match `dwarfdump --uuid dist/OpenArchi.app/Contents/MacOS/openarchi`; run `codesign --verify --deep --strict dist/OpenArchi.app` when available. Ad-hoc signing changes bytes, so do not use raw executable comparison as a freshness test.
- Launch the packaged app and perform a native smoke test for substantial UI/GPU changes when the environment permits. If blocked, state the exact blocker and mark desktop UI as unverified. Compilation, RawInput tests and screenshots of an older build are not manual desktop verification.
- For macOS icon changes, follow [the app icon packaging and Dock verification guide](docs/development/macos-app-icon.md). Finder showing the new `.icns` does not prove the running Dock icon is correct; eframe needs the same source image configured through `ViewportBuilder::with_icon`, and Dock must be checked after launching one fresh packaged instance.
- Report the exact app path, UUID/signature result, checks actually run, test counts, manual steps performed and remaining unknowns. If packaging fails, mark the bundle unverified; never imply an older app is current.

#### UI language and localization checks

Any change that adds or edits user-visible text, menus, tooltips, status messages,
counts, dialogs, Quick Options, or the language system is not complete until both
English and Simplified Chinese have been exercised on the final packaged app.

- Treat a language dropdown or a partially translated window as incomplete. Every
  visible main-window surface must follow the active language: title bar, top
  menus, toolbar, Info Box, toolbox, view tabs, counts, status bar, Quick Options
  bar and palette, and every dialog or panel touched by the change.
- Verify both entry points independently: the title-bar selector and
  `View > Language`. Both must apply immediately and save the same preference.
- Switch English → 简体中文 → English on one fresh `dist/OpenArchi.app` instance.
  Check the actual accessibility tree or screenshots for representative labels,
  not just the selector text.
- Restart the app after choosing 简体中文 and confirm the first frame is Chinese.
  Restore the user's previous preference when finished.
- Confirm switching changes presentation only: project elements, selection,
  dirty state, undo/redo history and `Undo`/`Redo` availability must not change.
- Add or update both `locales/en.json` and `locales/zh-CN.json` without duplicate
  keys, and add focused regressions for changed labels, dynamic status messages,
  counts, and Quick Options or dialog text. Run `./scripts/cargo.sh test
  --all-targets` and `python3 scripts/ac29-workflow.py verify --force`.
- Record the entry points, surfaces checked, restart result, test counts, final
  bundle UUID/signature and any unverified dialogs in
  `docs/features/ui-language.md` and `docs/validation.md`. If a surface was not
  checked in both languages, mark it unknown instead of claiming full localization.

### 5. Close the loop

- Update the focused feature note and relevant `docs/validation.md`, README usage/limitations or research entry in the same change. Keep feature claims within the evidence and verification performed.
- Review `git diff` and `git status`; preserve unrelated work. Do not commit, tag, publish, push or release unless explicitly asked.
- For comparisons, define a finite checklist with matching start model, units, settings, actions and tolerances. Record each claim as `pass`, `fail` or `unknown` with both sides' evidence. REA verification passes only the declared claims and supported evidence dimensions; it is not global equivalence.

## REA investigation notes

- Reuse an existing target/session and matching Evidence; bind or switch targets explicitly only when needed. Record target identity/hash, provider, Evidence IDs, coverage and limitations. Close only sessions opened for this investigation; do not close unrelated user sessions.
- Start from one concrete seed (tool/menu command, control, gesture, error, symbol or string). Use focused traces and inline Evidence; ask for a smaller view or export when results are large instead of repeating the same analysis.
- Static traces, decompiled pseudocode and inferred call edges are analysis evidence, not recovered original source or proof of runtime causality. Keep unresolved/indirect paths explicit. Record unknowns with the authority and probe needed to resolve them.
- For bounded runtime captures, declare the exact target, minimal scenario, steps, duration/stop condition, expected file effects and cleanup. Record before/after state; probe only relevant positive, invalid, cancel, undo/redo and save/reopen cases. Distinguish UI captures, input actions, process events, logs and measured model results.
- Use native static/artifact analysis only if it answers a remaining feature question. Do not inspect unrelated plugins or broaden target access by analogy.

## Security and network work

- OpenArchi is a native desktop prototype. Do not invoke PentAGI for offline-only work.
- For a new network service, public endpoint or security-sensitive form, define the authorized target/scope and run suitable checks only on that target. Use PentAGI when available and appropriate; otherwise report its absence and the local checks performed without implying a penetration test.

## Reusable one-request brief

The user may give only the first line; fill known context from the conversation and workspace. Ask only about consequential unknowns that evidence cannot resolve.

```text
Goal / exact Archicad command or gesture:
Target version/build and authorized scope:
Initial view, selection, model, units and settings:
Expected pointer/keyboard sequence and visible feedback:
Confirm/cancel, undo/redo and persistence expectations:
Existing evidence and unknowns:
Acceptance checklist / tolerances:
OpenArchi implementation and reversible assumptions:
Checks / packaged app path and freshness / remaining gaps:
UI language: both entry points, main-window coverage, restart persistence, no model/undo change
```
