# Desktop safety: close guard, recovery and native file dialog

Date: 2026-10-18
Scope: OpenArchi 0.2 plan stages 1–2 (desktop usability)
Status: implemented, automatically tested and desktop-verified on the final
macOS bundle, including UUID `7266453C-B925-3BDB-86E4-C6AD60BC9A61`.
Windows real-device startup and native file-dialog interaction remain
unverified on this machine.

## Goal and interaction contract

- Closing a dirty project must never silently discard work. The native close
  request is cancelled, and the app shows **Save and close / Discard and close /
  Cancel**.
- **Save and close** validates and writes the current path, clears the recovery
  file, then closes. A write error keeps the window and model intact and shows
  the error in the dialog.
- **Discard and close** removes the recovery file and closes without saving.
- **Cancel** only dismisses the dialog; project, selection, dirty state and
  undo/redo history are unchanged.
- A clean project closes normally.
- While a project is dirty, a recovery JSON document is written at most once
  per 60 seconds to the platform configuration directory. A successful save,
  explicit discard, Open or New clears it.
- At startup, an existing recovery document produces a modal offer to
  **Recover** or **Discard**. A recovered project remains dirty until it is
  saved, so the close guard still protects it.
- The Save/Open dialog now has a native **Browse…** button. The path field stays
  available as an explicit fallback and for automated tests.

## Evidence

| Claim | Type | Source | Limits |
|---|---|---|---|
| egui/eframe can cancel a native close with `ViewportCommand::CancelClose` | readable source | eframe 0.36.2 `epi.rs` `App::ui` documentation; egui `ViewportCommand` | The OS close event itself was not observed on Windows. |
| Dirty close is cancelled and prompts | automated test | `app::ui_tests::dirty_close_request_is_cancelled_and_opens_prompt` | Headless `RawInput` with `ViewportEvent::Close`, not a real window. |
| Clean close is allowed | automated test | `app::ui_tests::clean_close_request_is_not_cancelled` | Same limit. |
| Save clears dirty/recovery and writes valid JSON | automated test | `app::ui_tests::save_project_clears_dirty_and_recovery_file` | Temporary file on this machine. |
| Recovery stays dirty until save | automated test | `app::ui_tests::recovered_project_remains_dirty_until_saved` | Does not simulate a process crash. |
| Recovery round-trip and invalid file handling | automated test | `src/recovery.rs` tests | No real crash or power loss. |
| Dirty window close shows the guard | runtime observation | Final `dist/OpenArchi.app`, 2026-10-18; Chinese and English prompts read in the AX tree and screenshots | macOS only; the OS close event itself was not inspected on Windows. |
| Cancel preserves model, selection and undo state | runtime observation | Final bundle, 2026-10-18; English close prompt cancelled with 12 elements and unchanged selection | `Undo`/`Redo` availability was unchanged. |
| Save and close writes and exits | runtime observation | Final bundle, 2026-10-18; 14 elements, `next_id=15`, recovery removed, process exited | Saved to a temporary project file. |
| Discard and close clears recovery without writing | runtime observation | Final bundle, 2026-10-18; app exited and `recovery.openarchi.json` was absent | No project file comparison was needed because discard does not write. |
| Recovery prompt after a real crash | runtime observation | Final bundle, 2026-10-18; `killall -9 openarchi` after a dirty edit, relaunch offered Recover/Discard | Process kill, not a power loss. |
| Recovered project stays dirty | runtime observation | Final bundle, 2026-10-18; Recover produced 13 elements, dirty marker and enabled Undo | Selection and camera are not recovered. |
| Native `rfd` save panel selects a path | runtime observation | Final bundle, 2026-10-18; Browse chose `/private/tmp/openarchi-0.2-smoke.openarchi.json`, then the egui dialog saved it | The native open panel was not re-exercised after the final build. |
| `Cmd+Q` bypasses the in-app close prompt | runtime observation | Final bundle, 2026-10-18; `Cmd+Q` exited directly but left a recovery file that was offered on the next launch | macOS AppKit quit path; recovery still protects the dirty model. |
| Current bundle prompts on a dirty window close | runtime observation | UUID `7266453C-B925-3BDB-86E4-C6AD60BC9A61`, 2026-10-18; Chinese `未保存的更改` dialog with Save/Discard/Cancel after a 3.005 m wall edit | macOS only; buttons were clicked by window coordinates because egui modal buttons are absent from the AX tree. |
| Cancel preserves the edited model | runtime observation | Same bundle; cancelled the dialog and observed 12 elements, dirty marker, enabled Undo and height `3.005 m` | Selection and camera were already unchanged; only the close prompt was dismissed. |
| Discard and close leaves no recovery file | runtime observation | Same bundle; process exited, `recovery.openarchi.json` was removed, no save panel appeared | No project file was written; discard has no save path. |
| Relaunch after discard is clean | runtime observation | Same bundle; Chinese first frame, no recovery prompt, 12 elements, Undo/Redo disabled | Confirms the previous recovery file was not offered again. |

## Assumptions and exclusions

- Recovery is a single last-writer-wins file, not a history of versions.
- Recovery is written on the UI thread; very large projects may pause briefly.
- A recovery file from a previous version that fails validation is ignored
  with an error string rather than partially loaded.
- No automatic recovery of selection, camera or panel state.
- Windows real-device font/close/dialog behaviour still requires a manual smoke
  test and must remain `unknown` in release notes until captured.

## Acceptance checklist

| Scenario | Expected | Result |
|---|---|---|
| Dirty project, native close | Close cancelled; Save/Discard/Cancel shown | pass (desktop, Chinese and English; current bundle rechecked) |
| Cancel | Model, selection, undo/redo unchanged | pass (desktop; current bundle preserved 3.005 m, dirty and Undo) |
| Save and close | JSON written, dirty false, recovery removed, app closes | pass (desktop) |
| Discard and close | Recovery removed, no project write | pass (desktop; current bundle exited and removed recovery) |
| Dirty project left for 60 s | Recovery file exists | pass (unit-level write; timer not desktop-timed; a commit was observed to write immediately) |
| Startup with recovery file | Recover/Discard prompt | pass (desktop, including recovery after `killall -9`) |
| Browse… in Save/Open | Native panel selects a path | pass for Save (desktop); Open panel unknown |
| Windows Chinese first frame | No missing glyphs | unknown (no Windows runtime here) |

## Known limitation

On macOS, `Cmd+Q` is handled by the application menu and exits without showing
the in-app close prompt. The dirty project is still written to the recovery
sidecar before exit and is offered on the next launch, so it is not silently
lost; the prompt itself is not shown for that path. Intercepting the AppKit quit
event is not part of this change and remains `unknown` as a product decision.
