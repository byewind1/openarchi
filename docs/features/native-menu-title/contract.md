# Native menu bar and file-name window title (macOS)

Date: 2026-10-25 · Status: implemented; partial desktop acceptance · Scope: document window chrome on all platforms; native menu bar on macOS.

## User goal and request

1. OpenArchi's menus currently render as an in-window egui strip. On macOS, Archicad 29
   puts its menus in the system menu bar outside the window; OpenArchi must do the same.
2. The window title bar must show the open project's file name (未命名/Untitled when none),
   not the software name.

Evidence types: user directive (2026-10-25 request); macOS platform convention / Apple HIG
for menu-bar placement and document-titled windows (public documentation). No AC29 runtime
capture was made for this task; the observable convention is enough for placement.

## Behavior contract

### Menu bar (macOS)

- Start state: packaged `dist/OpenArchi.app` launches; no project opened yet.
- Sequence: the system menu bar shows, in order: 应用菜单 (OpenArchi icon/app name),
  文件/文件, Edit/编辑, View/视图, Design/设计, Document/文档, Options/选项,
  Teamwork/协作, Window/窗口, Help/帮助 — the same menu tree, order and labels as the
  previous in-window egui strip, localized by the active language.
- The in-window "commands" strip (menus + quick Undo/Redo/Fit/view buttons) is hidden on
  macOS. The standard toolbar, view tabs, Quick Options and Info Box stay unchanged.
  Windows/Linux keep the in-window egui menus exactly as before.
- Application menu contains: About OpenArchi, Services (system-managed submenu),
  Hide OpenArchi ⌘H, Hide Others ⌥⌘H, Show All, Quit OpenArchi ⌘Q.
- Keyboard equivalents are attached to Undo ⌘Z, Redo ⇧⌘Z, Open ⌘O, Save ⌘S,
  Duplicate ⌘D, Element Settings ⌘T, Layers… ⌘L, Floor Plan F2, 3D Perspective F3,
  Show selection/Marquee in 3D F5 plus the system items. Because AppKit delivers a menu
  key-equivalent before the app sees the key, these actions duplicate the existing egui
  shortcut handlers, including the `egui_wants_keyboard_input` guard (typing in a text
  field still swallows them).
- Cut/Copy/Paste/Select All intentionally get NO key equivalents: egui text widgets need
  ⌘X/⌘C/⌘V/⌘A delivered as key events; intercepting them globally would break text
  editing. The menu items remain clickable and run the element-level
  cut/copy/paste/duplicate commands, exactly like the egui Edit menu today.
- Menu-item state mirrors app state: Undo/Redo disabled when history is empty; check
  marks on view toggles (Trace, Guide Lines, Construction Grid, Element Snap, Toolbox,
  Navigator, Quick Options bar/palette, Info Box, languages, Floor Plan/Split/3D).
  The menu is rebuilt when the language or any of those states changes.
- Clicking a native item runs the same code paths as the egui menu (same editor/app
  methods, same commit/undo semantics for project-affecting toggles like Guide Lines).
  Deeply interactive in-window submenus that cannot be mirrored as static items map to
  their dialog/palette equivalents: Trace Options → Trace & Reference palette,
  Guides → snap/guide toggles + Set Snap Point Values… dialog, Grid & Editing Plane
  Options → Grids dialog, Document ▸ Layers/Pen Sets/Model View/Graphic Overrides/
  Design Options/Renovation → their existing dialogs. Preview-only commands open the
  same command-preview dialog as before.
- While any modal is open (file/new dialog, settings, recovery, close prompt, command
  preview…), native menu commands are ignored except About and Quit, matching the
  egui-shortcut gating. Quit runs the existing unsaved-changes flow.

### Window title

- The macOS window title shows the current document name: the saved/opened file name
  with `.json` and a trailing `.openarchi` stripped (e.g. `院子.openarchi.json` →
  `院子`); localized 未命名/Untitled until a file has actually been opened or saved.
  It never shows the software name. Re-applied on open/save/new and on language change.
- The in-window header strip that repeated the document name and showed the version and
  language picker is removed on all platforms. The native window title is the sole
  document-name display; language changes remain available from View ▸ Language.

## Model / undo / persistence effects

None beyond existing commands: menu actions call existing operations; window title and
menu bar are presentation-only and are not part of undo history or the saved project.

## Evidence summary

- Menu tree, labels, order, enabled/checked rules: readable source (`src/app/shell.rs`,
  `src/app/native_menu.rs` shares keys with the egui tree).
- Placement convention: public documentation (macOS HIG). Not a runtime capture of AC29.
- Partial desktop verification of the packaged app (menu bar, immediate language
  switching and opening the project dialog) is recorded in `docs/validation.md` for
  this date. File-name title after opening, restart persistence and other checklist
  items remain unknown.

## Assumptions (reversible)

- In-window menus are retained on Windows/Linux; a hidden `in_window_menus` flag keeps
  them testable in headless ui_tests on macOS.
- Unsaved projects use a single localized Untitled name (no counter).
- The window title does not append an edited marker. Dirty state remains available in
  the status and editing surfaces; the removed title strip no longer carries a dirty dot.

## Exclusions

- No per-window NSWindow proxy icon / document path pop-up, no full-screen menu-item
  auto-added by the app (macOS adds Show Tab/Enter Full Screen itself), no custom
  menu search, no touch-bar. Preview-only commands stay previews.

## Acceptance checklist

1. **pass — desktop:** packaged app exposes the ten OpenArchi menus in the macOS
   system menu bar; Chinese labels were observed with zh-CN active.
2. **partial — desktop:** native View ▸ Language switched the main window and menu
   labels to English and back to 简体中文 immediately. Restart persistence was not
   checked for this feature-specific run.
3. **partial — desktop:** native File ▸ 打开 JSON… opened the app's project-open
   dialog. The prefilled path `/tmp/oa-menu-test-home/Documents/courtyard.openarchi.json`
   did not exist (`No such file or directory`), so opening a document and the resulting
   file-name window title remain **unknown**.
4. **unknown — desktop:** native Edit ▸ Undo and disabled state were not exercised.
5. **unknown — desktop:** new-project title reset and ⌘Q unsaved-close flow were not
   exercised.
6. **unknown — desktop:** ⌘C/⌘V/⌘A in text fields were not exercised.
7. **partial — automated:** all-target Rust tests passed on macOS, but no Windows/Linux
   build or runtime check was performed in this acceptance run.
8. **pass — desktop, 2026-10-25:** final packaged app has no in-window document/version/
   language header; the OS title bar is the only document-name title. The sole language
   entry point is View ▸ Language, as requested.

Automated verification and current desktop evidence: `docs/validation.md`, section
“2026-10-25 原生菜单栏与文档标题验收”.

The duplicate in-window header was removed on 2026-10-25 after desktop review; the
language picker is now only in View ▸ Language. Current layout acceptance is recorded
in `docs/validation.md` under “去除重复的应用内标题行”.
